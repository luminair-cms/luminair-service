use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::domain::repository::RepositoryError;

// ApiSucess is a wrapper around a response that includes a status code.

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

// ApiError is a wrapper around a response that includes a status code.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    InternalServerError(String),
    UnprocessableEntity(String),
    ConflictWithServerState(String),
    NotFound
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::InternalServerError(e.to_string())
    }
}

impl From<RepositoryError> for ApiError {
    fn from(value: RepositoryError) -> Self {
        match value {
            RepositoryError::NotFound => {
                Self::NotFound
            }
            RepositoryError::ValidationFailed(cause) => {
                Self::UnprocessableEntity(cause)
            }
            RepositoryError::UniqueViolation(cause) => {
                Self::ConflictWithServerState(cause)
            }
            RepositoryError::DatabaseError(cause) => {
                tracing::error!("{:?}", cause);
                Self::InternalServerError("Database server error".to_string())
            }
        }
    }
}

/*
impl From<CreateAuthorError> for ApiError {
    fn from(e: CreateAuthorError) -> Self {
        match e {
            CreateAuthorError::Duplicate { name } => {
                Self::UnprocessableEntity(format!("author with name {} already exists", name))
            }
            CreateAuthorError::Unknown(cause) => {
                tracing::error!("{:?}\n{}", cause, cause.backtrace());
                Self::InternalServerError("Internal server error".to_string())
            }
        }
    }
}

impl From<AuthorNameEmptyError> for ApiError {
    fn from(_: AuthorNameEmptyError) -> Self {
        Self::UnprocessableEntity("author name cannot be empty".to_string())
    }
}

 */

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        use ApiError::*;

        match self {
            InternalServerError(e) => {
                tracing::error!("{}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponseBody::new_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal server error".to_string(),
                    )),
                )
                    .into_response()
            }
            UnprocessableEntity(message) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ApiResponseBody::new_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    message,
                )),
            )
                .into_response(),
            ConflictWithServerState(message) => (
                StatusCode::CONFLICT,
                Json(ApiResponseBody::new_error(
                    StatusCode::CONFLICT, 
                    message
                ))
            )
                .into_response(),
            NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

// Generic response structure shared by all API responses.

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiResponseBody<T: Serialize + PartialEq> {
    pub status_code: u16,
    pub data: T,
}

impl ApiResponseBody<ApiErrorData> {
    pub fn new_error(status_code: StatusCode, message: String) -> Self {
        Self {
            status_code: status_code.as_u16(),
            data: ApiErrorData { message },
        }
    }
}

/// The response data format for all error responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiErrorData {
    pub message: String,
}

