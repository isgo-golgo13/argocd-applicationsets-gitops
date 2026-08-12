//! # API Error Types
//!
//! Unified error handling for the GraphQL API layer.

use async_graphql::{Error as GraphQLError, ErrorExtensions};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

/// API-level errors
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Entity not found: {entity_type} with id '{id}'")]
    NotFound { entity_type: String, id: String },

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Invalid UUID format: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[error("Persistence error: {0}")]
    Persistence(#[from] drone_persistence::PersistenceError),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Rate limited: retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl ApiError {
    /// Get HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::InvalidInput(_) | Self::InvalidUuid(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Persistence(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get error code for GraphQL extensions
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "NOT_FOUND",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::InvalidUuid(_) => "INVALID_UUID",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::RateLimited { .. } => "RATE_LIMITED",
            Self::Persistence(_) => "PERSISTENCE_ERROR",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

impl ErrorExtensions for ApiError {
    fn extend(&self) -> GraphQLError {
        GraphQLError::new(self.to_string()).extend_with(|_, e| {
            e.set("code", self.error_code());
            e.set("status", self.status_code().as_u16());

            match self {
                Self::NotFound { entity_type, id } => {
                    e.set("entity_type", entity_type.as_str());
                    e.set("entity_id", id.as_str());
                }
                Self::RateLimited { retry_after_secs } => {
                    e.set("retry_after_secs", *retry_after_secs);
                }
                _ => {}
            }
        })
    }
}

// Note: We don't implement From<ApiError> for GraphQLError because async_graphql
// has a blanket impl for T: Display + Send + Sync + 'static, which ApiError satisfies.
// Use ApiError's ErrorExtensions::extend() method to get a GraphQL error with extensions.

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = serde_json::json!({
            "error": {
                "message": self.to_string(),
                "code": self.error_code(),
            }
        });

        (status, axum::Json(body)).into_response()
    }
}

/// Result type alias for API operations
pub type ApiResult<T> = Result<T, ApiError>;
