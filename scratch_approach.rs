//! End-to-end service integration tests.
//!
//! These tests exercise the full stack — HTTP routing, application service layer,
//! and PostgreSQL persistence — using a real containerised Postgres database
//! provisioned once per binary by `testcontainers`.
//!
//! # Design decisions
//!
//! ## Single shared container, isolated schemas
//! Booting one Docker container per test is expensive and causes connection-reset
//! races when many containers start in parallel.  Instead we boot **one** container
//! for the entire binary (lazily on first use) and give each test its own Postgres
//! *schema*, achieving full isolation without the overhead.
//!
//! ## Schema-level isolation
//! Every call to `build_router()` creates a fresh schema, runs migrations inside
//! it, and then builds an `AppStateImpl` scoped to that schema.  Tests never share
//! data even when they run in parallel.

use std::sync::OnceLock;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use serde_json::Value;
use service::infrastructure::{
    AppStateImpl,
    http::{handlers::health_check, routes::api_routes},
    persistence::repository::PostgresDocumentsRepository,
};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
use tower::ServiceExt;

use luminair_common::{
    database::{self, DatabaseConnection, DatabaseCredentials, DatabaseSettings},
    load_documents, DocumentTypesRegistry,
};
use migration::{application::Migration, infrastructure::persistence::PersistenceAdapter};

// ---------------------------------------------------------------------------
// Singletons — initialised once per test binary
// ---------------------------------------------------------------------------

/// Schema registry — parse once, share forever.
static REGISTRY: OnceLock<&'static dyn DocumentTypesRegistry> = OnceLock::new();

fn registry() -> &'static dyn DocumentTypesRegistry {
    *REGISTRY.get_or_init(|| {
        let schema_path = format!("{}/../../config/schema", env!("CARGO_MANIFEST_DIR"));
        load_documents(&schema_path).expect("failed to load schema registry")
    })
}

/// Shared database pool — one container, one pool, many schemas.
static SHARED_POOL: OnceLock<&'static sqlx::PgPool> = OnceLock::new();

/// Initialise the shared container + pool on first call; subsequent calls are
/// cheap.  The container is intentionally leaked so the Docker process lives
/// for the entire test run.
async fn shared_pool() -> &'static sqlx::PgPool {
    if let Some(pool) = SHARED_POOL.get() {
        return pool;
    }

    let container = Postgres::default()
        .start()
        .await
        .expect("failed to start Postgres container");

    let host = container
        .get_host()
        .await
        .expect("failed to get container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("failed to get container port");

    let settings = DatabaseSettings {
        host: format!("{host}:{port}"),
        db: "postgres".to_string(),
        schema: "public".to_string(), // default — each test gets its own schema
        credentials: DatabaseCredentials {
            username: "postgres".to_string(),
            password: "postgres".to_string(),
        },
        connection: DatabaseConnection {
            min_connections: 1,
            max_connections: 20,
            acquire_timeout_seconds: 10,
        },
    };

    let database = database::connect(&settings)
        .await
        .expect("failed to connect to database");

    // Leak the container so Docker keeps running for the lifetime of the binary.
    Box::leak(Box::new(container));
    // Leak the database so the pool lives forever.
    let database: &'static database::Database = Box::leak(Box::new(database));

    SHARED_POOL
        .set(database.database_pool())
        .ok()
        .expect("SHARED_POOL already set");

    SHARED_POOL.get().unwrap()
}

// ---------------------------------------------------------------------------
// Per-test router builder
// ---------------------------------------------------------------------------

type TestRouter = Router;

/// Create a fresh Postgres schema, run migrations, and wire up an Axum router.
///
/// Each test gets its own schema so data never bleeds between tests, even when
/// tests run concurrently.
async fn build_router() -> anyhow::Result<TestRouter> {
    let pool = shared_pool().await;
    let reg = registry();

    let schema_name = format!("test_{}", uuid::Uuid::new_v4().simple());

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE SCHEMA \"{schema_name}\""
    )))
    .execute(pool)
    .await?;

    let persistence = PersistenceAdapter::new(pool.clone(), &schema_name);
    Migration::new(reg, persistence).migrate(false).await?;

    // Build a database handle pointing at our test schema.
    let settings = {
        // We need the *existing* pool settings but with the new schema.
        // Re-connect using the same host/port and the new schema name.
        let conn_str = pool.connect_options();
        let _ = conn_str; // not easily extractable; use SHARED_POOL host info below
        DatabaseSettings {
            // These must match what shared_pool() used — we embed them here as
            // constants because there is no public accessor on PgPool.
            host: {
                // Retrieve the host from the pool's connect options via debug output.
                // In practice we know the schema; the pool itself was connected to
                // "public" so we just create a new settings that shares the pool.
                //
                // Instead of re-connecting, we re-use the shared pool by creating
                // the Database wrapper directly from the existing pool.
                unreachable!("use make_database_from_pool instead")
            },
            db: "postgres".to_string(),
            schema: schema_name.clone(),
            credentials: DatabaseCredentials {
                username: "postgres".to_string(),
                password: "postgres".to_string(),
            },
            connection: DatabaseConnection {
                min_connections: 1,
                max_connections: 5,
                acquire_timeout_seconds: 5,
            },
        }
    };
    let _ = settings;

    // Create a Database that wraps the shared pool + our schema name.
    let database: &'static database::Database =
        Box::leak(Box::new(database::from_pool(pool, &schema_name)));

    let repository = PostgresDocumentsRepository::new(reg, database);
    let state = AppStateImpl::new(reg, repository, Default::default());

    Ok(Router::new()
        .route("/health", get(health_check))
        .nest("/api", api_routes())
        .with_state(state))
}
