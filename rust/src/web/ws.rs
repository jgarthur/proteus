use std::pin::Pin;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use tokio::sync::broadcast;
use tokio::time::{sleep_until, Instant as TokioInstant, Sleep};

use super::controller::{FramePayload, MetricsPayload, SimulationController};
use super::error::ApiError;
use super::types::{WsControlMessage, WsHelloMessage, WsMetricsMessage, API_VERSION};

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
                    &controller,
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
                    let frame = { frame_rx.borrow().clone() };
                    if let Some(frame) = frame {
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
                    let metrics = { metrics_rx.borrow().clone() };
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
                    if let Some(frame) = subscription.pending.take() {
                        if send_frame_now(&mut socket, subscription, frame).await.is_err() {
                            break;
                        }
                    }
                    subscription.timer = None;
                }
            }
        }
    }
}

async fn handle_client_message(
    socket: &mut WebSocket,
    message: Message,
    controller: &SimulationController,
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
                    if let Some(frame) = controller.current_frame() {
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
                    if let Some(metrics) = controller.current_metrics() {
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

enum WsAction {
    SubscribeFrames { max_fps: u32 },
    UnsubscribeFrames,
    SubscribeMetrics { every_n_ticks: u64 },
    UnsubscribeMetrics,
}

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

struct FrameSubscription {
    min_interval: Duration,
    last_sent_at: Option<Instant>,
    pending: Option<FramePayload>,
    timer: Option<Pin<Box<Sleep>>>,
}

impl FrameSubscription {
    fn new(max_fps: u32) -> Self {
        Self {
            min_interval: Duration::from_secs_f64(1.0 / f64::from(max_fps)),
            last_sent_at: None,
            pending: None,
            timer: None,
        }
    }

    fn has_timer(&self) -> bool {
        self.timer.is_some()
    }
}

struct MetricsSubscription {
    every_n_ticks: u64,
}

fn should_send_metrics(subscription: &MetricsSubscription, metrics: &MetricsPayload) -> bool {
    metrics.tick.is_multiple_of(subscription.every_n_ticks)
}

async fn try_send_frame(
    socket: &mut WebSocket,
    subscription: &mut FrameSubscription,
    frame: FramePayload,
) -> Result<(), axum::Error> {
    let now = Instant::now();
    match subscription.last_sent_at {
        None => send_frame_now(socket, subscription, frame).await,
        Some(last_sent_at) if now.duration_since(last_sent_at) >= subscription.min_interval => {
            send_frame_now(socket, subscription, frame).await
        }
        Some(last_sent_at) => {
            subscription.pending = Some(frame);
            let deadline = last_sent_at + subscription.min_interval;
            subscription.timer = Some(Box::pin(sleep_until(TokioInstant::from_std(deadline))));
            Ok(())
        }
    }
}

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

async fn frame_timer(subscription: Option<&mut FrameSubscription>) {
    if let Some(subscription) = subscription {
        if let Some(timer) = subscription.timer.as_mut() {
            timer.await;
        }
    }
}
