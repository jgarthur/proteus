//! Wires the feature-gated HTTP and WebSocket API around the simulation controller.

mod controller;
mod error;
mod types;
mod ws;

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::header::HeaderName;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{Html, Response};
use axum::routing::{get, post};
use axum::{serve as axum_serve, Json, Router};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;

pub use controller::{ControllerError, FramePayload, MetricsPayload, SimulationController};
use error::ApiError;
pub use types::{
    CreateSimulationRequest, CreateSimulationResponse, SimulationConfig, SimulationLifecycle,
    SimulationStatusResponse, API_VERSION, API_VERSION_HEADER,
};

use self::types::{CellQuery, CellRegionQuery, StepQuery};

const SMOKE_TEST_HTML: &str = include_str!("smoke_test.html");

/// Stores the shared controller handle for request handlers.
#[derive(Clone)]
struct AppState {
    controller: SimulationController,
}

/// Builds the full API router for one simulation controller.
pub fn router(controller: SimulationController) -> Router {
    let version_header_name = HeaderName::from_static("x-proteus-api-version");
    let version_header_value = HeaderValue::from_static(API_VERSION);

    Router::new()
        .route("/debug/smoke", get(smoke_test))
        .route("/v1/sim", post(create_sim).get(get_sim).delete(delete_sim))
        .route("/v1/sim/config", get(get_config))
        .route("/v1/sim/start", post(start_sim))
        .route("/v1/sim/pause", post(pause_sim))
        .route("/v1/sim/resume", post(resume_sim))
        .route("/v1/sim/step", post(step_sim))
        .route("/v1/sim/reset", post(reset_sim))
        .route("/v1/sim/metrics", get(get_metrics))
        .route("/v1/sim/cell", get(get_cell_by_query))
        .route("/v1/sim/cell/:index", get(get_cell_by_index))
        .route("/v1/sim/cells", get(get_cells))
        .route("/v1/ws", get(websocket))
        // Snapshot routes are intentionally deferred until the engine grows a settled,
        // non-speculative snapshot serialization boundary.
        .with_state(AppState { controller })
        .layer(CorsLayer::permissive())
        .layer(SetResponseHeaderLayer::if_not_present(
            version_header_name,
            version_header_value,
        ))
}

/// Starts the web server with a default controller instance.
pub async fn serve(listener: TcpListener) -> std::io::Result<()> {
    serve_with_controller(listener, SimulationController::new()).await
}

/// Starts the web server with an explicitly supplied controller.
pub async fn serve_with_controller(
    listener: TcpListener,
    controller: SimulationController,
) -> std::io::Result<()> {
    axum_serve(listener, router(controller)).await
}

/// Handles simulation creation requests.
async fn create_sim(
    State(state): State<AppState>,
    payload: Result<Json<CreateSimulationRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateSimulationResponse>), ApiError> {
    let payload = json_body(payload)?;
    let config = payload.resolve().map_err(ControllerError::InvalidConfig)?;
    let response = state.controller.create(config).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

/// Returns the current simulation status.
async fn get_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.status().await?))
}

/// Destroys the current simulation instance.
async fn delete_sim(State(state): State<AppState>) -> Result<StatusCode, ApiError> {
    state.controller.destroy().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Returns the current simulation config.
async fn get_config(State(state): State<AppState>) -> Result<Json<SimulationConfig>, ApiError> {
    Ok(Json(state.controller.config().await?))
}

/// Transitions the simulation from created to running.
async fn start_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.start().await?))
}

/// Pauses a running simulation.
async fn pause_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.pause().await?))
}

/// Resumes a paused simulation.
async fn resume_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.resume().await?))
}

/// Advances a paused simulation by a fixed number of ticks.
async fn step_sim(
    State(state): State<AppState>,
    query: Result<Query<StepQuery>, QueryRejection>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    let StepQuery { count } = parse_query(query)?;
    Ok(Json(state.controller.step(count.unwrap_or(1)).await?))
}

/// Rebuilds the simulation from its original config.
async fn reset_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.reset().await?))
}

/// Returns the latest observer metrics snapshot.
async fn get_metrics(
    State(state): State<AppState>,
) -> Result<Json<crate::observe::MetricsSnapshot>, ApiError> {
    Ok(Json(state.controller.metrics().await?))
}

/// Returns one cell inspection by flat index.
async fn get_cell_by_index(
    State(state): State<AppState>,
    Path(index): Path<usize>,
) -> Result<Json<crate::observe::CellInspection>, ApiError> {
    Ok(Json(state.controller.inspect_cell_by_index(index).await?))
}

/// Returns one cell inspection by grid coordinates.
async fn get_cell_by_query(
    State(state): State<AppState>,
    query: Result<Query<CellQuery>, QueryRejection>,
) -> Result<Json<crate::observe::CellInspection>, ApiError> {
    let CellQuery { x, y } = parse_query(query)?;
    Ok(Json(
        state.controller.inspect_cell_by_coordinates(x, y).await?,
    ))
}

/// Returns a bounded rectangular batch of cell inspections.
async fn get_cells(
    State(state): State<AppState>,
    query: Result<Query<CellRegionQuery>, QueryRejection>,
) -> Result<Json<Vec<crate::observe::CellInspection>>, ApiError> {
    let CellRegionQuery { x, y, w, h } = parse_query(query)?;
    Ok(Json(state.controller.inspect_region(x, y, w, h).await?))
}

/// Upgrades one connection into the WebSocket control/data stream.
async fn websocket(State(state): State<AppState>, websocket: WebSocketUpgrade) -> Response {
    websocket.on_upgrade(move |socket| ws::handle_socket(socket, state.controller))
}

/// Serves a minimal browser-based simulator viewer for visual smoke tests.
async fn smoke_test() -> Html<&'static str> {
    Html(SMOKE_TEST_HTML)
}

/// Unwraps a JSON request body or converts parser failures into API errors.
fn json_body<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|err| ApiError::bad_request(err.body_text()))
}

/// Unwraps query parameters or converts parser failures into API errors.
fn parse_query<T>(query: Result<Query<T>, QueryRejection>) -> Result<T, ApiError> {
    query
        .map(|Query(value)| value)
        .map_err(|err| ApiError::bad_request(err.body_text()))
}
