#![cfg(feature = "web")]

use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use proteus::web::{
    router, CreateSimulationRequest, SimulationController, API_VERSION, API_VERSION_HEADER,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;

fn app() -> (SimulationController, axum::Router) {
    let controller = SimulationController::new();
    let app = router(controller.clone());
    (controller, app)
}

#[tokio::test]
async fn rest_lifecycle_flow_and_inspection_work() {
    let (_controller, app) = app();

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/v1/sim",
            json!({
                "width": 2,
                "height": 1,
                "seed": 7,
                // Keep ambient pools at zero so these payload assertions stay focused on
                // the seeded program state rather than steady-state background initialization.
                "r_energy": 0.0,
                "r_mass": 0.0,
                "seed_programs": [{
                    "x": 0,
                    "y": 0,
                    "code": [80, 100],
                    "free_energy": 3,
                    "free_mass": 2
                }],
                "seed_environment": [{
                    "x": 1,
                    "y": 0,
                    "free_energy": 7,
                    "free_mass": 8,
                    "bg_radiation": 9,
                    "bg_mass": 10
                }]
            }),
        ))
        .await
        .expect("create request should succeed");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    assert_eq!(create_response.headers()[API_VERSION_HEADER], API_VERSION);
    let create_json = response_json(create_response).await;
    assert_eq!(create_json["status"], "created");
    assert_eq!(create_json["tick"], 0);

    let status_response = app
        .clone()
        .oneshot(empty_request(Method::GET, "/v1/sim"))
        .await
        .expect("status request should succeed");
    assert_eq!(status_response.status(), StatusCode::OK);
    let status_json = response_json(status_response).await;
    assert_eq!(status_json["population"], 1);
    assert_eq!(status_json["total_energy"], 19);
    assert_eq!(status_json["total_mass"], 22);

    let config_response = app
        .clone()
        .oneshot(empty_request(Method::GET, "/v1/sim/config"))
        .await
        .expect("config request should succeed");
    let config_json = response_json(config_response).await;
    assert_eq!(config_json["width"], 2);
    assert_eq!(config_json["seed_programs"][0]["code"], json!([80, 100]));
    assert_eq!(config_json["seed_environment"][0]["bg_radiation"], 9);

    let cell_response = app
        .clone()
        .oneshot(empty_request(Method::GET, "/v1/sim/cell?x=0&y=0"))
        .await
        .expect("cell request should succeed");
    let cell_json = response_json(cell_response).await;
    assert_eq!(cell_json["program"]["disassembly"], json!(["nop", "boot"]));

    let environment_response = app
        .clone()
        .oneshot(empty_request(Method::GET, "/v1/sim/cell?x=1&y=0"))
        .await
        .expect("environment cell request should succeed");
    let environment_json = response_json(environment_response).await;
    assert_eq!(environment_json["free_energy"], 7);
    assert_eq!(environment_json["free_mass"], 8);
    assert_eq!(environment_json["bg_radiation"], 9);
    assert_eq!(environment_json["bg_mass"], 10);
    assert!(environment_json["program"].is_null());

    let step_from_created_response = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/step?count=1"))
        .await
        .expect("step from created request should succeed");
    assert_eq!(step_from_created_response.status(), StatusCode::OK);
    let step_from_created_json = response_json(step_from_created_response).await;
    assert_eq!(step_from_created_json["status"], "paused");
    assert_eq!(step_from_created_json["tick"], 1);

    let pause_response = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/pause"))
        .await
        .expect("pause request should succeed");
    assert_eq!(pause_response.status(), StatusCode::OK);
    let pause_json = response_json(pause_response).await;
    assert_eq!(pause_json["status"], "paused");
    let paused_tick = pause_json["tick"].as_u64().expect("tick should be a u64");

    let step_response = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/step?count=2"))
        .await
        .expect("step request should succeed");
    let step_json = response_json(step_response).await;
    assert_eq!(step_json["status"], "paused");
    assert_eq!(step_json["tick"], paused_tick + 2);

    let resume_response = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/resume"))
        .await
        .expect("resume request should succeed");
    let resume_json = response_json(resume_response).await;
    assert_eq!(resume_json["status"], "running");

    let pause_again_response = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/pause"))
        .await
        .expect("second pause request should succeed");
    let pause_again_json = response_json(pause_again_response).await;
    assert_eq!(pause_again_json["status"], "paused");

    // Once started, pause and resume name a desired end state and converge on
    // it, so a client may re-issue either one from a stale view of the lifecycle.
    let redundant_pause = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/pause"))
        .await
        .expect("pausing an already-paused simulation should succeed");
    assert_eq!(redundant_pause.status(), StatusCode::OK);
    assert_eq!(response_json(redundant_pause).await["status"], "paused");

    let resume_once = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/resume"))
        .await
        .expect("resume request should succeed");
    assert_eq!(response_json(resume_once).await["status"], "running");

    let redundant_resume = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/resume"))
        .await
        .expect("resuming an already-running simulation should succeed");
    assert_eq!(redundant_resume.status(), StatusCode::OK);
    assert_eq!(response_json(redundant_resume).await["status"], "running");

    // Start is not convergent: it boots out of `created` and nothing else.
    let restart = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/start"))
        .await
        .expect("restarting a running simulation should return an error response");
    assert_eq!(restart.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(restart).await["error"]["code"],
        "SIM_ALREADY_STARTED"
    );

    let settle = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/pause"))
        .await
        .expect("pause request should succeed");
    assert_eq!(response_json(settle).await["status"], "paused");

    let reset_response = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/reset"))
        .await
        .expect("reset request should succeed");
    let reset_json = response_json(reset_response).await;
    assert_eq!(reset_json["status"], "created");
    assert_eq!(reset_json["tick"], 0);

    let destroy_response = app
        .clone()
        .oneshot(empty_request(Method::DELETE, "/v1/sim"))
        .await
        .expect("destroy request should succeed");
    assert_eq!(destroy_response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn rest_errors_use_expected_status_codes_and_payloads() {
    let (_controller, app) = app();

    let missing_sim = app
        .clone()
        .oneshot(empty_request(Method::GET, "/v1/sim"))
        .await
        .expect("request should return an error response");
    assert_eq!(missing_sim.status(), StatusCode::NOT_FOUND);
    let missing_json = response_json(missing_sim).await;
    assert_eq!(missing_json["error"]["code"], "NO_SIM");

    let invalid_create = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/v1/sim",
            json!({
                "width": 0,
                "height": 1,
                "seed": 9
            }),
        ))
        .await
        .expect("request should return an error response");
    assert_eq!(invalid_create.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let invalid_json = response_json(invalid_create).await;
    assert_eq!(invalid_json["error"]["code"], "INVALID_CONFIG");

    let create_ok = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/v1/sim",
            json!({
                "width": 1,
                "height": 1,
                "seed": 1
            }),
        ))
        .await
        .expect("create request should succeed");
    assert_eq!(create_ok.status(), StatusCode::CREATED);

    let create_conflict = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/v1/sim",
            json!({
                "width": 1,
                "height": 1,
                "seed": 2
            }),
        ))
        .await
        .expect("request should return an error response");
    assert_eq!(create_conflict.status(), StatusCode::CONFLICT);
    let conflict_json = response_json(create_conflict).await;
    assert_eq!(conflict_json["error"]["code"], "SIM_ALREADY_EXISTS");

    let cell_oob = app
        .clone()
        .oneshot(empty_request(Method::GET, "/v1/sim/cell/9"))
        .await
        .expect("request should return an error response");
    assert_eq!(cell_oob.status(), StatusCode::BAD_REQUEST);
    let cell_json = response_json(cell_oob).await;
    assert_eq!(cell_json["error"]["code"], "CELL_OUT_OF_BOUNDS");

    let region_too_large = app
        .clone()
        .oneshot(empty_request(
            Method::GET,
            "/v1/sim/cells?x=0&y=0&w=11&h=10",
        ))
        .await
        .expect("request should return an error response");
    assert_eq!(region_too_large.status(), StatusCode::BAD_REQUEST);
    let region_json = response_json(region_too_large).await;
    assert_eq!(region_json["error"]["code"], "REGION_TOO_LARGE");

    // `created` sits outside the convergent pair: the simulation has never run,
    // so neither control can name a state for it to settle into.
    for control in ["pause", "resume"] {
        let from_created = app
            .clone()
            .oneshot(empty_request(Method::POST, &format!("/v1/sim/{control}")))
            .await
            .expect("request should return an error response");
        assert_eq!(
            from_created.status(),
            StatusCode::CONFLICT,
            "{control} from created"
        );
        assert_eq!(
            response_json(from_created).await["error"]["code"],
            "SIM_NOT_STARTED",
            "{control} from created"
        );
    }

    let start_from_created = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/start"))
        .await
        .expect("starting from created should succeed");
    assert_eq!(start_from_created.status(), StatusCode::OK);

    let step_while_running = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/step?count=1"))
        .await
        .expect("request should return an error response");
    assert_eq!(step_while_running.status(), StatusCode::CONFLICT);
    let step_running_json = response_json(step_while_running).await;
    assert_eq!(step_running_json["error"]["code"], "SIM_NOT_PAUSED");

    let step_wrong_state = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/step?count=0"))
        .await
        .expect("request should return an error response");
    assert_eq!(step_wrong_state.status(), StatusCode::BAD_REQUEST);
    let step_json = response_json(step_wrong_state).await;
    assert_eq!(step_json["error"]["code"], "BAD_REQUEST");
}

#[tokio::test]
async fn smoke_test_page_is_served() {
    let (_controller, app) = app();

    let response = app
        .oneshot(empty_request(Method::GET, "/debug/smoke"))
        .await
        .expect("smoke page request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "text/html; charset=utf-8"
    );
    let body = response_text(response).await;
    assert!(body.contains("Proteus Smoke Test"));
    assert!(body.contains("/v1/ws"));
}

#[tokio::test]
async fn websocket_subscriptions_stream_current_state_and_report_errors() {
    let controller = SimulationController::new();
    let config = CreateSimulationRequest {
        width: 2,
        height: 1,
        seed: 11,
        // Keep the initial frame/metrics deterministic for transport assertions by disabling
        // steady-state ambient background seeding in this test.
        r_energy: Some(0.0),
        r_mass: Some(0.0),
        d_energy: None,
        d_mass: None,
        t_cap: None,
        maintenance_rate: None,
        maintenance_exponent: None,
        local_action_exponent: None,
        n_synth: None,
        inert_grace_ticks: None,
        p_spawn: None,
        mutation_base_log2: None,
        mutation_background_log2: None,
        seed_programs: vec![serde_json::from_value(json!({
            "x": 0,
            "y": 0,
            "code": [80],
            "free_energy": 5,
            "free_mass": 1
        }))
        .expect("seed program should deserialize")],
        seed_environment: Vec::new(),
    }
    .resolve()
    .expect("config should resolve");
    controller
        .create(config)
        .await
        .expect("simulation should be created");

    let server = spawn_server(controller.clone()).await;
    let url = format!("ws://{}/v1/ws", server.addr);
    let (mut websocket, _) = connect_async(url).await.expect("websocket should connect");

    let hello = next_text_message(&mut websocket).await;
    assert_eq!(hello["type"], "hello");
    assert_eq!(hello["api_version"], API_VERSION);

    websocket
        .send(Message::Text(
            r#"{"subscribe":"frames","max_fps":30}"#.into(),
        ))
        .await
        .expect("frame subscription should send");
    let frame = next_binary_message(&mut websocket).await;
    assert_eq!(frame.len(), 32);
    assert_eq!(&frame[0..8], &0_u64.to_le_bytes());
    assert_eq!(&frame[8..12], &2_u32.to_le_bytes());
    assert_eq!(&frame[12..16], &1_u32.to_le_bytes());
    assert_eq!(frame[16], 0b011);
    assert_eq!(frame[18], 0);
    assert_eq!(frame[19], 5);
    assert_eq!(frame[20], 1);
    assert_eq!(frame[21], 0);
    assert_eq!(frame[22], 0);
    assert_eq!(frame[23], 0);

    websocket
        .send(Message::Text(
            r#"{"subscribe":"metrics","every_n_ticks":1}"#.into(),
        ))
        .await
        .expect("metrics subscription should send");
    let metrics = next_text_message(&mut websocket).await;
    assert_eq!(metrics["type"], "metrics");
    assert_eq!(metrics["tick"], 0);
    assert_eq!(metrics["population"], 1);
    assert_eq!(metrics["total_energy"], 5);
    assert_eq!(metrics["total_mass"], 2);

    websocket
        .send(Message::Text(r#"{"subscribe":"unknown"}"#.into()))
        .await
        .expect("invalid subscription should send");
    let error = next_text_message_of_type(&mut websocket, "error").await;
    assert_eq!(error["type"], "error");
    assert_eq!(error["code"], "INVALID_SUBSCRIPTION");

    controller.destroy().await.expect("destroy should succeed");
    let close = tokio::time::timeout(Duration::from_secs(1), websocket.next())
        .await
        .expect("close event should arrive");
    match close {
        Some(Ok(Message::Close(_))) | None => {}
        other => panic!("expected websocket close, got {other:?}"),
    }

    server.handle.abort();
}

#[tokio::test]
async fn websocket_destroy_discards_a_throttled_pending_frame() {
    let controller = SimulationController::new();
    let config = CreateSimulationRequest {
        width: 1,
        height: 1,
        seed: 17,
        r_energy: Some(0.0),
        r_mass: Some(0.0),
        d_energy: None,
        d_mass: None,
        t_cap: None,
        maintenance_rate: None,
        maintenance_exponent: None,
        local_action_exponent: None,
        n_synth: None,
        inert_grace_ticks: None,
        p_spawn: None,
        mutation_base_log2: None,
        mutation_background_log2: None,
        seed_programs: vec![serde_json::from_value(json!({
            "x": 0,
            "y": 0,
            "code": [80],
            "free_energy": 5,
            "free_mass": 1
        }))
        .expect("seed program should deserialize")],
        seed_environment: Vec::new(),
    }
    .resolve()
    .expect("config should resolve");
    controller
        .create(config)
        .await
        .expect("simulation should be created");

    let server = spawn_server(controller.clone()).await;
    let url = format!("ws://{}/v1/ws", server.addr);
    let (mut websocket, _) = connect_async(url).await.expect("websocket should connect");
    let _hello = next_text_message(&mut websocket).await;

    websocket
        .send(Message::Text(
            r#"{"subscribe":"frames","max_fps":1}"#.into(),
        ))
        .await
        .expect("frame subscription should send");
    let initial_frame = next_binary_message(&mut websocket).await;
    assert_eq!(&initial_frame[0..8], &0_u64.to_le_bytes());

    controller
        .step(1)
        .await
        .expect("simulation should publish a second frame");
    tokio::time::sleep(Duration::from_millis(50)).await;

    controller.destroy().await.expect("destroy should succeed");
    let close = tokio::time::timeout(Duration::from_secs(1), websocket.next())
        .await
        .expect("close event should arrive");
    match close {
        Some(Ok(Message::Close(_))) | None => {}
        Some(Ok(Message::Binary(_))) => {
            panic!("pending binary frame was delivered across destroy acknowledgement")
        }
        other => panic!("expected websocket close, got {other:?}"),
    }

    server.handle.abort();
}

#[tokio::test]
async fn cumulative_event_totals_survive_sampling_and_reset_epochs() {
    let controller = SimulationController::new();
    let config = CreateSimulationRequest {
        width: 1,
        height: 1,
        seed: 23,
        r_energy: Some(0.0),
        r_mass: Some(100.0),
        d_energy: Some(1.0),
        d_mass: Some(1.0),
        t_cap: None,
        maintenance_rate: None,
        maintenance_exponent: None,
        local_action_exponent: None,
        n_synth: None,
        inert_grace_ticks: None,
        p_spawn: Some(1.0),
        mutation_base_log2: None,
        mutation_background_log2: None,
        seed_programs: Vec::new(),
        seed_environment: Vec::new(),
    }
    .resolve()
    .expect("config should resolve");
    controller
        .create(config.clone())
        .await
        .expect("simulation should be created");

    let rest_app = router(controller.clone());
    let server = spawn_server(controller.clone()).await;
    let url = format!("ws://{}/v1/ws", server.addr);
    let (mut websocket, _) = connect_async(url).await.expect("websocket should connect");
    let _hello = next_text_message(&mut websocket).await;

    websocket
        .send(Message::Text(
            r#"{"subscribe":"metrics","every_n_ticks":5}"#.into(),
        ))
        .await
        .expect("metrics subscription should send");
    let initial = next_text_message(&mut websocket).await;
    assert_eq!(initial["epoch"], 0);
    assert_eq!(initial["tick"], 0);
    assert_eq!(initial["event_totals"]["births"], 0);

    controller
        .step(5)
        .await
        .expect("simulation should step five ticks");
    let sampled = next_metrics_message_at_tick(&mut websocket, 5).await;
    assert_eq!(sampled["epoch"], 0);
    assert_eq!(sampled["tick"], 5);
    assert_eq!(sampled["births"], 0);
    assert_eq!(sampled["event_totals"]["births"], 1);
    assert_eq!(sampled["event_totals"]["boot_births"], 0);
    assert_eq!(sampled["event_totals"]["spawn_births"], 1);

    let latest = controller
        .metrics()
        .await
        .expect("latest metrics should be available");
    assert_eq!(latest.epoch, 0);
    assert_eq!(latest.tick, 5);
    assert_eq!(latest.event_totals.births, 1);
    assert_eq!(
        latest.event_totals.births,
        latest.event_totals.boot_births + latest.event_totals.spawn_births
    );

    let rest_response = rest_app
        .oneshot(empty_request(Method::GET, "/v1/sim/metrics"))
        .await
        .expect("metrics request should succeed");
    assert_eq!(rest_response.status(), StatusCode::OK);
    let rest_metrics = response_json(rest_response).await;
    assert_eq!(rest_metrics["epoch"], sampled["epoch"]);
    assert_eq!(rest_metrics["tick"], sampled["tick"]);
    assert_eq!(rest_metrics["event_totals"], sampled["event_totals"]);

    controller.reset().await.expect("simulation should reset");
    let reset = next_metrics_message_at_epoch(&mut websocket, 1).await;
    assert_eq!(reset["epoch"], 1);
    assert_eq!(reset["tick"], 0);
    assert_eq!(reset["event_totals"]["births"], 0);
    assert_eq!(reset["event_totals"]["deaths"], 0);
    assert_eq!(reset["event_totals"]["mutations"], 0);

    controller.destroy().await.expect("destroy should succeed");
    controller
        .create(config)
        .await
        .expect("replacement simulation should be created");
    let replacement = controller
        .metrics()
        .await
        .expect("replacement metrics should be available");
    assert_eq!(replacement.epoch, 2);
    assert_eq!(replacement.tick, 0);
    assert_eq!(replacement.event_totals, Default::default());

    controller.destroy().await.expect("destroy should succeed");
    server.handle.abort();
}

struct SpawnedServer {
    addr: std::net::SocketAddr,
    handle: JoinHandle<()>,
}

async fn spawn_server(controller: SimulationController) -> SpawnedServer {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have an address");
    let handle = tokio::spawn(async move {
        proteus::web::serve_with_controller(listener, controller)
            .await
            .expect("server should run");
    });

    SpawnedServer { addr, handle }
}

fn empty_request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("request should build")
}

fn json_request(method: Method, uri: &str, value: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(value.to_string()))
        .expect("request should build")
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("body should contain JSON")
}

async fn response_text(response: axum::response::Response) -> String {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    String::from_utf8(bytes.to_vec()).expect("body should contain utf-8 text")
}

async fn next_text_message(
    websocket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    let message = tokio::time::timeout(Duration::from_secs(1), websocket.next())
        .await
        .expect("message should arrive")
        .expect("stream should stay open")
        .expect("websocket message should be valid");
    let Message::Text(text) = message else {
        panic!("expected text websocket message");
    };
    serde_json::from_str(text.as_ref()).expect("text message should be JSON")
}

async fn next_binary_message(
    websocket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Vec<u8> {
    let message = tokio::time::timeout(Duration::from_secs(1), websocket.next())
        .await
        .expect("message should arrive")
        .expect("stream should stay open")
        .expect("websocket message should be valid");
    let Message::Binary(bytes) = message else {
        panic!("expected binary websocket message");
    };
    bytes.to_vec()
}

async fn next_text_message_of_type(
    websocket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    expected_type: &str,
) -> Value {
    for _ in 0..3 {
        let message = next_text_message(websocket).await;
        if message["type"] == expected_type {
            return message;
        }
    }

    panic!("expected websocket message of type {expected_type}");
}

async fn next_metrics_message_at_tick(
    websocket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    expected_tick: u64,
) -> Value {
    for _ in 0..3 {
        let message = next_text_message(websocket).await;
        if message["type"] == "metrics" && message["tick"] == expected_tick {
            return message;
        }
    }

    panic!("expected metrics websocket message at tick {expected_tick}");
}

/// Waits for the first metrics message belonging to `expected_epoch`.
///
/// A reset does not retract metrics already published for the previous epoch, so
/// a message from the outgoing epoch can still be in flight when `reset` returns.
/// Those are valid output and are skipped; overshooting the epoch is not.
async fn next_metrics_message_at_epoch(
    websocket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    expected_epoch: u64,
) -> Value {
    for _ in 0..4 {
        let message = next_text_message(websocket).await;
        if message["type"] != "metrics" {
            continue;
        }

        let epoch = message["epoch"]
            .as_u64()
            .expect("metrics messages should carry a numeric epoch");
        if epoch < expected_epoch {
            continue;
        }

        assert_eq!(
            epoch, expected_epoch,
            "metrics epoch overshot {expected_epoch}"
        );
        return message;
    }

    panic!("expected metrics websocket message at epoch {expected_epoch}");
}
