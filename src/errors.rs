// src/errors.rs

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum AppError {
    // Database errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Record not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    // Auth errors
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Invalid token")]
    InvalidToken,

    // Validation errors
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    // External service errors
    #[error("Monnify API error: {0}")]
    MonnifyError(String),

    #[error("Email error: {0}")]
    EmailError(String),

    // Business logic errors
    #[error("Insufficient wallet balance: available {available}, required {required}")]
    InsufficientBalance { available: f64, required: f64 },

    #[error("Payroll already processed for this period")]
    PayrollAlreadyProcessed,

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Unauthorized(_) | AppError::InvalidToken => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::Validation(_) | AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::InsufficientBalance { .. } | AppError::PayrollAlreadyProcessed => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = json!({
            "error": {
                "code": status.as_u16(),
                "message": self.to_string(),
            }
        });
        (status, Json(body)).into_response()
    }
}

// Convenience alias
pub type AppResult<T> = Result<T, AppError>;
