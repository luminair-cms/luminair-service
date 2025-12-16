use anyhow::Context;
use axum::Router;
use axum::routing::get;
use axum_prometheus::PrometheusMetricLayer;

use tokio::net;
use crate::infrastructure::{AppState, http::handlers::data::{find_all_documents, find_document_by_id}};
use handlers::documents::{documents_metadata, one_document_metadata};
use crate::infrastructure::http::handlers::{health_check, hello_world_handler};

mod api;
mod handlers;

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
    pub async fn new(state: impl AppState, config: HttpServerConfig<'_>) -> anyhow::Result<Self> {
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

fn api_routes<S: AppState>() -> Router<S> {
    Router::new()
        .route("/hello", get(hello_world_handler::<S>))
        .route("/meta/documents", get(documents_metadata::<S>))
        .route("/meta/documents/{id}", get(one_document_metadata::<S>))
        .route("/data/documents/{document_id}", get(find_all_documents::<S>))
        .route("/data/documents/{document_id}/{id}", get(find_document_by_id::<S>))
}
