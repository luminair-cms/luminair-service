#![allow(dead_code)]

use std::sync::OnceLock;

pub use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
};
pub use serde_json::Value;
pub use service::infrastructure::{
    AppStateImpl,
    http::{handlers::health_check, routes::api_routes},
    persistence::repository::PostgresDocumentsRepository,
};
pub use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
pub use tower::ServiceExt;

pub use luminair_common::{
    DocumentTypesRegistry,
    database::{self, DatabaseConnection, DatabaseCredentials, DatabaseSettings},
    load_documents,
};
pub use migration::{application::Migration, infrastructure::persistence::PersistenceAdapter};

// ---------------------------------------------------------------------------
// Registry — initialised once per test binary
// ---------------------------------------------------------------------------

static REGISTRY: OnceLock<&'static dyn DocumentTypesRegistry> = OnceLock::new();

pub fn registry() -> &'static dyn DocumentTypesRegistry {
    *REGISTRY.get_or_init(|| {
        let schema_path = format!("{}/../../config/schema", env!("CARGO_MANIFEST_DIR"));
        load_documents(&schema_path).expect("failed to load schema registry")
    })
}

// ---------------------------------------------------------------------------
// Test harness helpers
// ---------------------------------------------------------------------------

/// Boot a fresh Postgres container, apply migrations, and return the pool + guard.
pub async fn start_postgres() -> anyhow::Result<(&'static database::Database, impl Drop)> {
    let reg = registry();
    let container = Postgres::default().start().await?;

    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let schema_name = format!("test_{}", uuid::Uuid::new_v4().simple());

    let settings = DatabaseSettings {
        host: format!("{host}:{port}"),
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
    };

    let database = database::Database::new(&settings).await?;
    let database = Box::leak(Box::new(database));
    let pool = database.database_pool();

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE SCHEMA \"{schema_name}\""
    )))
    .execute(pool)
    .await?;

    let persistence = PersistenceAdapter::new(pool.clone(), &schema_name);
    Migration::new(reg, persistence).migrate(false).await?;

    Ok((database, container))
}

pub type TestRouter = Router;

/// Build a fully wired Axum router backed by a fresh isolated database.
pub async fn build_router() -> anyhow::Result<(TestRouter, impl Drop)> {
    let reg = registry();
    let (database, container) = start_postgres().await?;
    let repository = PostgresDocumentsRepository::new(reg, database);
    let state = AppStateImpl::new(reg, repository, Default::default());
    let router = Router::new()
        .route("/health", get(health_check))
        .nest("/api", api_routes())
        .with_state(state);
    Ok((router, container))
}

// ---------------------------------------------------------------------------
// HTTP primitives
// ---------------------------------------------------------------------------

pub async fn get_json(router: &TestRouter, uri: &str) -> anyhow::Result<(StatusCode, Value)> {
    let response = router
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty())?)
        .await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await?;
    let json = serde_json::from_slice(&bytes)?;
    Ok((status, json))
}

pub async fn post_json(
    router: &TestRouter,
    uri: &str,
    body: &str,
) -> anyhow::Result<(StatusCode, axum::http::HeaderMap, Vec<u8>)> {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))?,
        )
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await?
        .to_vec();
    Ok((status, headers, bytes))
}

pub async fn put_json(
    router: &TestRouter,
    uri: &str,
    body: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))?,
        )
        .await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await?;
    let json = serde_json::from_slice(&bytes)?;
    Ok((status, json))
}

/// POST to create a document; returns the Location URI (without query string).
pub async fn create_document(
    router: &TestRouter,
    collection: &str,
    body: &str,
) -> anyhow::Result<String> {
    let (status, headers, bytes) =
        post_json(router, &format!("/api/documents/{collection}"), body).await?;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create_document failed ({collection}): {}",
        String::from_utf8_lossy(&bytes)
    );
    let loc = headers
        .get("location")
        .expect("missing Location header")
        .to_str()?
        .to_string();
    Ok(loc)
}

/// POST to `{document_uri}/publish`; returns Ok(()).
pub async fn publish_document(router: &TestRouter, document_uri: &str) -> anyhow::Result<()> {
    let (status, _, bytes) = post_json(router, &format!("{document_uri}/publish"), "{}").await?;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "publish failed: {}",
        String::from_utf8_lossy(&bytes)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

pub async fn create_brand(router: &TestRouter, uid: &str, name: &str) -> anyhow::Result<String> {
    create_document(
        router,
        "brands",
        &format!(r#"{{"data": {{"uid": "{uid}", "name": "{name}"}}}}"#),
    )
    .await
}

pub async fn create_partner(
    router: &TestRouter,
    idno: &str,
    legal_entity: &str,
) -> anyhow::Result<String> {
    create_document(
        router,
        "partners",
        &format!(r#"{{"data": {{"idno": "{idno}", "legal_entity": "{legal_entity}"}}}}"#),
    )
    .await
}

pub async fn create_partner_category(
    router: &TestRouter,
    uid: &str,
    priority: i32,
) -> anyhow::Result<String> {
    create_document(
        router,
        "partner-categories",
        &format!(
            r#"{{"data": {{"uid": "{uid}", "name": {{"en": "Category {uid}"}}, "priority": {priority}}}}}"#
        ),
    )
    .await
}
