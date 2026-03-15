use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use super::controller::ControllerError;
use super::types::{ErrorBody, ErrorEnvelope, WsErrorMessage};

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST",
            message: message.into(),
        }
    }

    pub fn ws_error(code: &'static str, message: impl Into<String>) -> WsErrorMessage {
        WsErrorMessage {
            kind: "error",
            code,
            message: message.into(),
        }
    }
}

impl From<ControllerError> for ApiError {
    fn from(error: ControllerError) -> Self {
        match error {
            ControllerError::NoSim => Self {
                status: StatusCode::NOT_FOUND,
                code: "NO_SIM",
                message: "No simulation exists.".to_owned(),
            },
            ControllerError::SimAlreadyExists => Self {
                status: StatusCode::CONFLICT,
                code: "SIM_ALREADY_EXISTS",
                message: "A simulation already exists. DELETE /v1/sim first.".to_owned(),
            },
            ControllerError::SimNotRunning => Self {
                status: StatusCode::CONFLICT,
                code: "SIM_NOT_RUNNING",
                message: "This operation requires a running simulation.".to_owned(),
            },
            ControllerError::SimNotPaused => Self {
                status: StatusCode::CONFLICT,
                code: "SIM_NOT_PAUSED",
                message: "This operation requires a paused simulation.".to_owned(),
            },
            ControllerError::SimNotCreated => Self {
                status: StatusCode::CONFLICT,
                code: "SIM_NOT_CREATED",
                message: "This operation requires the simulation to be in the created state."
                    .to_owned(),
            },
            ControllerError::InvalidConfig(message) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "INVALID_CONFIG",
                message,
            },
            ControllerError::CellOutOfBounds => Self {
                status: StatusCode::BAD_REQUEST,
                code: "CELL_OUT_OF_BOUNDS",
                message: "The requested cell is outside the simulation grid.".to_owned(),
            },
            ControllerError::RegionTooLarge { area } => Self {
                status: StatusCode::BAD_REQUEST,
                code: "REGION_TOO_LARGE",
                message: format!("Requested region contains {area} cells; the maximum is 100."),
            },
            ControllerError::BadRequest(message) => Self {
                status: StatusCode::BAD_REQUEST,
                code: "BAD_REQUEST",
                message,
            },
            ControllerError::Internal(message) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "INTERNAL_ERROR",
                message,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: ErrorBody {
                    code: self.code,
                    message: self.message,
                    status: self.status.as_u16(),
                },
            }),
        )
            .into_response()
    }
}
