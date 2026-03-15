mod controller;
mod error;
mod types;
mod ws;

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::header::HeaderName;
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
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

#[derive(Clone)]
struct AppState {
    controller: SimulationController,
}

pub fn router(controller: SimulationController) -> Router {
    let version_header_name = HeaderName::from_static("x-proteus-api-version");
    let version_header_value = HeaderValue::from_static(API_VERSION);

    Router::new()
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

pub async fn serve(listener: TcpListener) -> std::io::Result<()> {
    serve_with_controller(listener, SimulationController::new()).await
}

pub async fn serve_with_controller(
    listener: TcpListener,
    controller: SimulationController,
) -> std::io::Result<()> {
    axum_serve(listener, router(controller)).await
}

async fn create_sim(
    State(state): State<AppState>,
    payload: Result<Json<CreateSimulationRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateSimulationResponse>), ApiError> {
    let payload = json_body(payload)?;
    let config = payload.resolve().map_err(ControllerError::InvalidConfig)?;
    let response = state.controller.create(config).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

async fn get_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.status().await?))
}

async fn delete_sim(State(state): State<AppState>) -> Result<StatusCode, ApiError> {
    state.controller.destroy().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_config(State(state): State<AppState>) -> Result<Json<SimulationConfig>, ApiError> {
    Ok(Json(state.controller.config().await?))
}

async fn start_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.start().await?))
}

async fn pause_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.pause().await?))
}

async fn resume_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.resume().await?))
}

async fn step_sim(
    State(state): State<AppState>,
    query: Result<Query<StepQuery>, QueryRejection>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    let StepQuery { count } = parse_query(query)?;
    Ok(Json(state.controller.step(count.unwrap_or(1)).await?))
}

async fn reset_sim(
    State(state): State<AppState>,
) -> Result<Json<SimulationStatusResponse>, ApiError> {
    Ok(Json(state.controller.reset().await?))
}

async fn get_metrics(
    State(state): State<AppState>,
) -> Result<Json<crate::observe::MetricsSnapshot>, ApiError> {
    Ok(Json(state.controller.metrics().await?))
}

async fn get_cell_by_index(
    State(state): State<AppState>,
    Path(index): Path<usize>,
) -> Result<Json<crate::observe::CellInspection>, ApiError> {
    Ok(Json(state.controller.inspect_cell_by_index(index).await?))
}

async fn get_cell_by_query(
    State(state): State<AppState>,
    query: Result<Query<CellQuery>, QueryRejection>,
) -> Result<Json<crate::observe::CellInspection>, ApiError> {
    let CellQuery { x, y } = parse_query(query)?;
    Ok(Json(
        state.controller.inspect_cell_by_coordinates(x, y).await?,
    ))
}

async fn get_cells(
    State(state): State<AppState>,
    query: Result<Query<CellRegionQuery>, QueryRejection>,
) -> Result<Json<Vec<crate::observe::CellInspection>>, ApiError> {
    let CellRegionQuery { x, y, w, h } = parse_query(query)?;
    Ok(Json(state.controller.inspect_region(x, y, w, h).await?))
}

async fn websocket(State(state): State<AppState>, websocket: WebSocketUpgrade) -> Response {
    websocket.on_upgrade(move |socket| ws::handle_socket(socket, state.controller))
}

fn json_body<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|err| ApiError::bad_request(err.body_text()))
}

fn parse_query<T>(query: Result<Query<T>, QueryRejection>) -> Result<T, ApiError> {
    query
        .map(|Query(value)| value)
        .map_err(|err| ApiError::bad_request(err.body_text()))
}
