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
                "seed_programs": [{
                    "x": 0,
                    "y": 0,
                    "code": [80, 100],
                    "free_energy": 3,
                    "free_mass": 2
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
    assert_eq!(status_json["total_energy"], 3);
    assert_eq!(status_json["total_mass"], 4);

    let config_response = app
        .clone()
        .oneshot(empty_request(Method::GET, "/v1/sim/config"))
        .await
        .expect("config request should succeed");
    let config_json = response_json(config_response).await;
    assert_eq!(config_json["width"], 2);
    assert_eq!(config_json["seed_programs"][0]["code"], json!([80, 100]));

    let cell_response = app
        .clone()
        .oneshot(empty_request(Method::GET, "/v1/sim/cell?x=0&y=0"))
        .await
        .expect("cell request should succeed");
    let cell_json = response_json(cell_response).await;
    assert_eq!(cell_json["program"]["disassembly"], json!(["nop", "boot"]));

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

    let step_wrong_state = app
        .clone()
        .oneshot(empty_request(Method::POST, "/v1/sim/start"))
        .await
        .expect("request should return an error response");
    assert_eq!(step_wrong_state.status(), StatusCode::OK);

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
        r_energy: None,
        r_mass: None,
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
