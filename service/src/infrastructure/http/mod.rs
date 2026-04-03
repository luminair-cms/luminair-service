use anyhow::Context;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Extension, Router};
use axum_prometheus::PrometheusMetricLayer;

use crate::infrastructure::http::handlers::health_check;
use crate::infrastructure::http::querystring::QueryStringConfig;
use crate::infrastructure::http::routes::api_routes;
use crate::infrastructure::{
    http::handlers::data::{find_all_documents, find_document_by_id},
    AppState,
};
use handlers::documents::{documents_metadata, one_document_metadata};
use serde_querystring::ParseMode;
use tokio::net;

mod api;
mod handlers;
mod querystring;
pub mod routes;

/// Configuration for the HTTP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpServerConfig<'a> {
    pub port: &'a str,
}

/// The application's HTTP server. The underlying HTTP package is opaque to module consumers.
pub struct HttpServer {
    router: axum::Router,
    listener: net::TcpListener,
}

impl HttpServer {
    /// Returns a new HTTP server bound to the port specified in `config`.
    pub async fn new<S: AppState>(state: S, config: HttpServerConfig<'_>) -> anyhow::Result<Self> {
        let trace_layer = tower_http::trace::TraceLayer::new_for_http().make_span_with(
            |request: &axum::extract::Request<_>| {
                let uri = request.uri().to_string();
                tracing::info_span!("http_request", method = ?request.method(), uri)
            },
        );
        // see: https://github.com/metrics-rs/metrics
        // see: https://github.com/Ptrskay3/axum-prometheus
        let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

        let router = Router::new()
            .route("/health", get(health_check))
            .nest("/api", api_routes())
            .route("/metrics", get(|| async move { metric_handle.render() }))
            .layer(Extension(
                QueryStringConfig::new(ParseMode::Brackets).ehandler(|err| {
                    (StatusCode::BAD_REQUEST, err.to_string()) // return type should impl IntoResponse
                }),
            ))
            .layer(trace_layer)
            .layer(prometheus_layer)
            .with_state(state);

        let listener = net::TcpListener::bind(format!("0.0.0.0:{}", config.port))
            .await
            .with_context(|| format!("failed to listen on {}", config.port))?;

        Ok(Self { router, listener })
    }

    /// Runs the HTTP server.
    pub async fn run(self) -> anyhow::Result<()> {
        tracing::debug!("listening on {}", self.listener.local_addr().unwrap());
        axum::serve(self.listener, self.router)
            .await
            .context("received error from running server")?;
        Ok(())
    }
}
