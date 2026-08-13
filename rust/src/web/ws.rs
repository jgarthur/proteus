//! Handles the WebSocket control channel for frames and metrics streaming.

use std::pin::Pin;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use tokio::sync::{broadcast, watch};
use tokio::time::{sleep_until, Instant as TokioInstant, Sleep};

use super::controller::{FramePayload, MetricsPayload, SimulationController};
use super::error::ApiError;
use super::types::{WsControlMessage, WsHelloMessage, WsMetricsMessage, API_VERSION};

/// Runs one WebSocket session against the shared simulation controller.
pub async fn handle_socket(mut socket: WebSocket, controller: SimulationController) {
    if send_json(
        &mut socket,
        &WsHelloMessage {
            kind: "hello",
            api_version: API_VERSION,
        },
    )
    .await
    .is_err()
    {
        return;
    }

    let mut frame_rx = controller.frame_receiver();
    let mut metrics_rx = controller.metrics_receiver();
    let mut destroy_rx = controller.destroy_receiver();
    let mut frame_subscription = None::<FrameSubscription>;
    let mut metrics_subscription = None::<MetricsSubscription>;

    loop {
        tokio::select! {
            // Closing promptly on destroy, rather than after another frame or
            // metrics round trip. This ordering is a latency preference only:
            // the destroy guarantee comes from resolving throttled frames out of
            // the live stream, which the controller clears before notifying us.
            biased;
            destroy = destroy_rx.recv() => {
                match destroy {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        let _ = socket.close().await;
                    }
                    Err(broadcast::error::RecvError::Closed) => {}
                }
                break;
            }
            maybe_message = socket.recv() => {
                let Some(Ok(message)) = maybe_message else {
                    break;
                };

                if handle_client_message(
                    &mut socket,
                    message,
                    &mut frame_rx,
                    &mut metrics_rx,
                    &mut frame_subscription,
                    &mut metrics_subscription,
                ).await.is_err() {
                    break;
                }
            }
            changed = frame_rx.changed(), if frame_subscription.is_some() => {
                if changed.is_err() {
                    break;
                }

                if let Some(subscription) = frame_subscription.as_mut() {
                    if let Some(frame) = take_due_frame(&mut frame_rx) {
                        if try_send_frame(&mut socket, subscription, frame).await.is_err() {
                            break;
                        }
                    }
                }
            }
            changed = metrics_rx.changed(), if metrics_subscription.is_some() => {
                if changed.is_err() {
                    break;
                }

                if let Some(subscription) = metrics_subscription.as_ref() {
                    let metrics = metrics_rx.borrow().clone();
                    if let Some(metrics) = metrics {
                        if should_send_metrics(subscription, &metrics)
                            && send_metrics(&mut socket, &metrics).await.is_err()
                        {
                            break;
                        }
                    }
                }
            }
            _ = frame_timer(frame_subscription.as_mut()), if frame_subscription.as_ref().is_some_and(FrameSubscription::has_timer) => {
                if let Some(subscription) = frame_subscription.as_mut() {
                    subscription.timer = None;
                    // Resolve the owed frame from the live stream instead of
                    // replaying a payload cached when the throttle engaged. The
                    // controller clears this stream before it publishes destroy,
                    // so a frame owed by a destroyed simulation reads as `None`
                    // and is dropped no matter which branch wins this select.
                    if let Some(frame) = take_due_frame(&mut frame_rx) {
                        if send_frame_now(&mut socket, subscription, frame).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Processes one client message and updates the active subscriptions.
async fn handle_client_message(
    socket: &mut WebSocket,
    message: Message,
    frame_rx: &mut tokio::sync::watch::Receiver<Option<FramePayload>>,
    metrics_rx: &mut tokio::sync::watch::Receiver<Option<MetricsPayload>>,
    frame_subscription: &mut Option<FrameSubscription>,
    metrics_subscription: &mut Option<MetricsSubscription>,
) -> Result<(), ()> {
    match message {
        Message::Text(text) => {
            let control =
                serde_json::from_str::<WsControlMessage>(text.as_ref()).map_err(|_| ())?;
            match validate_control(control) {
                Ok(WsAction::SubscribeFrames { max_fps }) => {
                    *frame_subscription = Some(FrameSubscription::new(max_fps));
                    // Mark the current frame seen when we send it immediately so
                    // `changed()` does not replay the same payload on the next loop.
                    let frame = {
                        let current = frame_rx.borrow_and_update();
                        current.clone()
                    };
                    if let Some(frame) = frame {
                        let subscription = frame_subscription
                            .as_mut()
                            .expect("frame subscription should exist");
                        send_frame_now(socket, subscription, frame)
                            .await
                            .map_err(|_| ())?;
                    }
                }
                Ok(WsAction::UnsubscribeFrames) => {
                    *frame_subscription = None;
                }
                Ok(WsAction::SubscribeMetrics { every_n_ticks }) => {
                    *metrics_subscription = Some(MetricsSubscription { every_n_ticks });
                    let metrics = {
                        let current = metrics_rx.borrow_and_update();
                        current.clone()
                    };
                    if let Some(metrics) = metrics {
                        send_metrics(socket, &metrics).await.map_err(|_| ())?;
                    }
                }
                Ok(WsAction::UnsubscribeMetrics) => {
                    *metrics_subscription = None;
                }
                Err(message) => {
                    send_json(socket, &ApiError::ws_error("INVALID_SUBSCRIPTION", message))
                        .await
                        .map_err(|_| ())?;
                }
            }
        }
        Message::Close(_) => return Err(()),
        Message::Ping(payload) => {
            socket.send(Message::Pong(payload)).await.map_err(|_| ())?;
        }
        Message::Pong(_) => {}
        Message::Binary(_) => {
            send_json(
                socket,
                &ApiError::ws_error(
                    "INVALID_SUBSCRIPTION",
                    "Client control messages must be JSON text frames.",
                ),
            )
            .await
            .map_err(|_| ())?;
        }
    }

    Ok(())
}

/// Represents the validated subscription action requested by a client.
enum WsAction {
    SubscribeFrames { max_fps: u32 },
    UnsubscribeFrames,
    SubscribeMetrics { every_n_ticks: u64 },
    UnsubscribeMetrics,
}

/// Validates one parsed WebSocket control message.
fn validate_control(control: WsControlMessage) -> Result<WsAction, String> {
    match (control.subscribe.as_deref(), control.unsubscribe.as_deref()) {
        (Some("frames"), None) => {
            let max_fps = control.max_fps.unwrap_or(30);
            if !(1..=60).contains(&max_fps) {
                return Err("frames subscriptions require max_fps between 1 and 60.".to_owned());
            }
            Ok(WsAction::SubscribeFrames { max_fps })
        }
        (Some("metrics"), None) => {
            let every_n_ticks = control.every_n_ticks.unwrap_or(1);
            if every_n_ticks == 0 {
                return Err(
                    "metrics subscriptions require every_n_ticks greater than zero.".to_owned(),
                );
            }
            Ok(WsAction::SubscribeMetrics { every_n_ticks })
        }
        (None, Some("frames")) => Ok(WsAction::UnsubscribeFrames),
        (None, Some("metrics")) => Ok(WsAction::UnsubscribeMetrics),
        _ => Err("Unknown or malformed subscription request.".to_owned()),
    }
}

/// Tracks frame-stream throttling state for one client.
///
/// Only the send deadline is retained. The frame owed at that deadline is read
/// back from the watch stream when the timer fires, so a destroy that clears the
/// stream also cancels the delivery.
struct FrameSubscription {
    min_interval: Duration,
    last_sent_at: Option<Instant>,
    timer: Option<Pin<Box<Sleep>>>,
}

impl FrameSubscription {
    /// Builds a new frame subscription with a requested FPS cap.
    fn new(max_fps: u32) -> Self {
        Self {
            min_interval: Duration::from_secs_f64(1.0 / f64::from(max_fps)),
            last_sent_at: None,
            timer: None,
        }
    }

    /// Reports whether a delayed frame send is currently scheduled.
    fn has_timer(&self) -> bool {
        self.timer.is_some()
    }

    /// Reports whether the FPS throttle currently permits an immediate send.
    fn is_due(&self, now: Instant) -> bool {
        match self.last_sent_at {
            None => true,
            Some(last_sent_at) => now.duration_since(last_sent_at) >= self.min_interval,
        }
    }

    /// Schedules the next send at the end of the current throttle interval.
    fn arm_timer(&mut self) {
        let Some(last_sent_at) = self.last_sent_at else {
            return;
        };
        let deadline = last_sent_at + self.min_interval;
        self.timer = Some(Box::pin(sleep_until(TokioInstant::from_std(deadline))));
    }
}

/// Reads the frame a client is owed, or `None` once destroy cleared the stream.
fn take_due_frame(frame_rx: &mut watch::Receiver<Option<FramePayload>>) -> Option<FramePayload> {
    frame_rx.borrow_and_update().clone()
}

/// Tracks the tick cadence for metrics updates.
struct MetricsSubscription {
    every_n_ticks: u64,
}

/// Decides whether a metrics payload matches the current subscription cadence.
fn should_send_metrics(subscription: &MetricsSubscription, metrics: &MetricsPayload) -> bool {
    metrics.tick.is_multiple_of(subscription.every_n_ticks)
}

/// Sends a frame immediately, or defers to the end of the throttle interval.
async fn try_send_frame(
    socket: &mut WebSocket,
    subscription: &mut FrameSubscription,
    frame: FramePayload,
) -> Result<(), axum::Error> {
    if subscription.is_due(Instant::now()) {
        return send_frame_now(socket, subscription, frame).await;
    }

    // Drop this payload and re-read the stream at the deadline. Holding no copy
    // is what lets destroy cancel the delivery by clearing the stream.
    subscription.arm_timer();
    Ok(())
}

/// Sends one frame payload immediately and updates throttle state.
async fn send_frame_now(
    socket: &mut WebSocket,
    subscription: &mut FrameSubscription,
    frame: FramePayload,
) -> Result<(), axum::Error> {
    subscription.last_sent_at = Some(Instant::now());
    subscription.timer = None;
    socket
        .send(Message::Binary(frame.bytes.as_ref().clone()))
        .await
}

/// Sends one metrics payload as a JSON WebSocket message.
async fn send_metrics(socket: &mut WebSocket, metrics: &MetricsPayload) -> Result<(), axum::Error> {
    send_json(
        socket,
        &WsMetricsMessage {
            kind: "metrics",
            metrics: (*metrics.snapshot).clone(),
        },
    )
    .await
}

/// Serializes one JSON value into a text WebSocket frame.
async fn send_json(
    socket: &mut WebSocket,
    value: &impl serde::Serialize,
) -> Result<(), axum::Error> {
    socket
        .send(Message::Text(
            serde_json::to_string(value).expect("WebSocket messages should serialize"),
        ))
        .await
}

/// Awaits the next scheduled frame-send deadline when throttling is active.
async fn frame_timer(subscription: Option<&mut FrameSubscription>) {
    if let Some(subscription) = subscription {
        if let Some(timer) = subscription.timer.as_mut() {
            timer.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use tokio::sync::watch;

    use super::{take_due_frame, FramePayload, FrameSubscription};

    fn frame(tick: u64) -> FramePayload {
        FramePayload {
            tick,
            bytes: Arc::new(vec![0, 1, 2, 3]),
        }
    }

    #[test]
    fn a_due_frame_resolves_to_nothing_once_destroy_clears_the_stream() {
        let (frame_tx, mut frame_rx) = watch::channel(Some(frame(1)));
        assert!(take_due_frame(&mut frame_rx).is_some());

        // Destroy clears the frame stream before it notifies sockets. A timer
        // that fires after that point therefore has no frame left to deliver,
        // which is what keeps a throttled frame off the wire across destroy.
        frame_tx
            .send(None)
            .expect("frame stream should accept the destroy clear");
        assert!(take_due_frame(&mut frame_rx).is_none());
    }

    #[test]
    fn a_due_frame_resolves_to_the_latest_published_value() {
        let (frame_tx, mut frame_rx) = watch::channel(Some(frame(1)));
        frame_tx
            .send(Some(frame(2)))
            .expect("frame stream should accept a newer frame");

        let due = take_due_frame(&mut frame_rx).expect("a frame should be owed");
        assert_eq!(due.tick, 2);
    }

    #[test]
    fn the_fps_throttle_defers_until_the_interval_elapses() {
        let mut subscription = FrameSubscription::new(1);
        let now = Instant::now();

        assert!(
            subscription.is_due(now),
            "the first frame sends immediately"
        );

        subscription.last_sent_at = Some(now);
        assert!(!subscription.is_due(now));
        assert!(!subscription.is_due(now + Duration::from_millis(999)));
        assert!(subscription.is_due(now + Duration::from_millis(1000)));
    }
}
