use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::application::error::ServiceError;

// ApiSuccess is a wrapper around a response that includes a status code.

#[derive(Debug, Clone)]
pub struct ApiSuccess<T: Serialize>(StatusCode, Json<T>);

impl<T: Serialize> ApiSuccess<T> {
    pub(crate) fn new(status: StatusCode, data: T) -> Self {
        ApiSuccess(status, Json(data))
    }
}

impl<T: Serialize> IntoResponse for ApiSuccess<T> {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

/// The error type returned by all API handlers.
///
/// Each variant maps to an HTTP status code in the [`IntoResponse`] impl.
/// Implements [`std::error::Error`] via `thiserror` so it participates in the
/// standard Rust error chain and can be inspected programmatically.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("Conflict: {0}")]
    ConflictWithServerState(String),

    #[error("Not found")]
    NotFound,
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::InternalServerError(e.to_string())
    }
}

impl From<ServiceError> for ApiError {
    fn from(value: ServiceError) -> Self {
        match value {
            ServiceError::DocumentTypeNotFound
            | ServiceError::DocumentNotFound
            | ServiceError::RelationNotFound(_) => Self::NotFound,
            ServiceError::NotOwningRelation(relation) => Self::UnprocessableEntity(
                format!("Relation is not an owning relation: {}", relation),
            ),
            ServiceError::Validation(cause) => Self::UnprocessableEntity(cause.to_string()),
            ServiceError::Conflict(cause) => Self::ConflictWithServerState(cause),
            ServiceError::Internal(internal) => internal.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        use ApiError::*;

        let (status, detail) = match self {
            InternalServerError(msg) => {
                tracing::error!("{}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "An internal server error occurred".to_string())
            }
            UnprocessableEntity(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            ConflictWithServerState(msg) => (StatusCode::CONFLICT, msg),
            NotFound => (StatusCode::NOT_FOUND, "The requested resource was not found".to_string()),
        };

        let problem = ProblemDetails::new(status, detail);
        (
            status,
            [("content-type", "application/problem+json")],
            Json(problem),
        )
            .into_response()
    }
}

/// Standard-compliant RFC 7807 / RFC 9457 Problem Details structure for HTTP API errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

impl ProblemDetails {
    pub fn new(status: StatusCode, detail: String) -> Self {
        Self {
            problem_type: "about:blank".to_string(),
            title: status.canonical_reason().unwrap_or("Unknown Error").to_string(),
            status: status.as_u16(),
            detail,
            instance: None,
        }
    }
}


