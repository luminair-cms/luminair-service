use luminair_common::{connect_to_database, load_documents};
use crate::infrastructure::persistence::PersistenceAdapter;
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

    let documents = load_documents(&settings.schema_config_path)?;
    println!("Configuration loaded");

    let database = connect_to_database(&settings.database).await?;
    println!("Connected to DB");

    let hello_service = HelloServiceAdapter::new(&database);
    let persistence = PersistenceAdapter::new(&database);

    let state = AppStateImpl::new(hello_service, documents, persistence);

    let server_config = HttpServerConfig {
        port: &settings.server_port,
    };
    let http_server = HttpServer::new(state, server_config).await?;
    http_server.run().await
}
