use std::sync::Arc;

use luminair_common::{database, load_documents};
use crate::domain::{AppState, HelloService};
use crate::infrastructure::persistence::PersistenceAdapter;
use crate::infrastructure::persistence::repository::PostgresDocumentRepository;
use crate::infrastructure::{AppStateImpl, HelloServiceAdapter};
use crate::infrastructure::http::{HttpServer, HttpServerConfig};
use crate::infrastructure::settings::Settings;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod domain;
mod infrastructure;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::from_env()?;

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let registry = load_documents(&settings.schema_config_path)?;
    tracing::debug!("Configuration loaded");

    let database = database::connect(&settings.database).await?;
    tracing::debug!("Connected to DB");

    let hello_service = Arc::new(HelloServiceAdapter::new(database));
    let repository = PostgresDocumentRepository::new(registry, database);
    let state = AppState::new(hello_service, registry, repository);

    let server_config = HttpServerConfig {
        port: &settings.server_port,
    };
    let http_server = HttpServer::new(state, server_config).await?;
    http_server.run().await
}
