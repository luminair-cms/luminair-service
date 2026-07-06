use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::Context;
use serde::Deserialize;
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};

#[derive(Clone, Debug)]
pub struct Database {
    database_pool: PgPool,
    database_schema: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    pub host: String,
    pub db: String,
    pub schema: String,
    pub credentials: DatabaseCredentials,
    pub connection: DatabaseConnection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConnection {
    pub min_connections: u32,
    pub max_connections: u32,
    pub acquire_timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseCredentials {
    pub username: String,
    pub password: String,
}

static DATABASE: OnceLock<Arc<Database>> = OnceLock::new();

pub async fn connect(settings: &DatabaseSettings) -> Result<&'static Database, anyhow::Error> {
    if let Some(db) = DATABASE.get() {
        return Ok(db.as_ref());
    }
    let database = Database::new(settings).await?;
    let _ = DATABASE.set(Arc::new(database));
    Ok(DATABASE.get().unwrap().as_ref())
}

impl Database {
    pub async fn new(settings: &DatabaseSettings) -> Result<Self, anyhow::Error> {
        let credentials = &settings.credentials;
        let mut host = settings.host.as_str();
        let mut port = 5432;
        if let Some((h, p)) = settings.host.split_once(':') {
            host = h;
            if let Ok(parsed_port) = p.parse::<u16>() {
                port = parsed_port;
            }
        }

        let pg_connect_options = PgConnectOptions::new()
            .host(host)
            .port(port)
            .username(&credentials.username)
            .password(&credentials.password)
            .database(&settings.db)
            .ssl_mode(PgSslMode::Prefer)
            .options([("search_path", settings.schema.as_str())]);

        let connection = &settings.connection;
        let pool = PgPoolOptions::new()
            .min_connections(connection.min_connections)
            .max_connections(connection.max_connections)
            .acquire_timeout(Duration::from_secs(connection.acquire_timeout_seconds))
            .connect_with(pg_connect_options)
            .await
            .with_context(|| {
                format!(
                    "failed to open database at {}/{}",
                    settings.host, settings.db
                )
            })?;

        Ok(Self {
            database_pool: pool,
            database_schema: settings.schema.to_owned(),
        })
    }

    pub fn database_pool(&self) -> &PgPool {
        &self.database_pool
    }

    pub fn database_schema(&self) -> &str {
        &self.database_schema
    }
}
