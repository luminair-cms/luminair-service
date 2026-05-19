use std::{ops::Deref, sync::Arc};

use anyhow::Error;
use axum::{extract::FromRequestParts, http::{StatusCode, request::Parts}, response::{IntoResponse, Response}};
use serde::de::DeserializeOwned;
use serde_querystring::ParseMode;

#[derive(Debug, Clone, Copy, Default)]
pub struct QueryString<T>(pub T);

impl<T, S> FromRequestParts<S> for QueryString<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let QueryStringConfig { mode, ehandler } = parts
            .extensions
            .get::<QueryStringConfig>()
            .cloned()
            .unwrap_or_default();

        let query = parts.uri.query().unwrap_or_default();
        let value = serde_querystring::from_str(query, mode).map_err(|e| {
            if let Some(ehandler) = ehandler {
                ehandler(e.into())
            } else {
                QueryStringError::default().into_response()
            }
        })?;
        Ok(QueryString(value))
    }
}

impl<T> Deref for QueryString<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub struct QueryStringConfig {
    mode: ParseMode,
    ehandler: Option<Arc<dyn Fn(Error) -> Response + Send + Sync>>,
}

impl Default for QueryStringConfig {
    fn default() -> Self {
        Self {
            mode: ParseMode::Duplicate,
            ehandler: None,
        }
    }
}

impl QueryStringConfig {
    pub fn new(mode: ParseMode) -> Self {
        Self {
            mode,
            ehandler: None,
        }
    }

    pub fn mode(mut self, mode: ParseMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn ehandler<F, R>(mut self, ehandler: F) -> Self
    where
        F: Fn(Error) -> R + Send + Sync + 'static,
        R: IntoResponse,
    {
        self.ehandler = Some(Arc::new(move |e| ehandler(e).into_response()));
        self
    }
}

#[derive(Debug)]
struct QueryStringError {
    status: StatusCode,
    body: String,
}

impl Default for QueryStringError {
    fn default() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: String::from("Failed to deserialize query string"),
        }
    }
}

impl IntoResponse for QueryStringError {
    fn into_response(self) -> Response {
        (self.status, self.body).into_response()
    }
}