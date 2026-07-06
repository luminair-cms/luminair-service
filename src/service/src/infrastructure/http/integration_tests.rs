use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use tower::ServiceExt; // for oneshot
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
use serde_json::Value;

use luminair_common::{
    database::{self, DatabaseSettings, DatabaseCredentials, DatabaseConnection},
    load_documents, DocumentTypesRegistry,
};
use migration::{
    application::Migration,
    infrastructure::persistence::PersistenceAdapter,
};
use crate::{
    infrastructure::{
        AppStateImpl,
        http::{handlers::health_check, routes::api_routes},
        persistence::repository::PostgresDocumentsRepository,
    },
};

// Start a PostgreSQL container and return the connected Database and the container guard.
async fn start_postgres(registry: &'static dyn DocumentTypesRegistry) -> anyhow::Result<(&'static database::Database, impl Drop)> {
    let container = Postgres::default()
        .start()
        .await?;

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

    let database = database::connect(&settings).await?;
    let pool = database.database_pool();

    // Create schema
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA \"{schema_name}\"")))
        .execute(pool)
        .await?;

    // Run migration
    let persistence = PersistenceAdapter::new(pool.clone(), &schema_name);
    Migration::new(registry, persistence)
        .migrate(false)
        .await?;

    Ok((database, container))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_http_integration_suite() -> anyhow::Result<()> {
    // 1. Load schema registry (already returns &'static dyn DocumentTypesRegistry)
    let schema_path = format!("{}/../../config/schema", env!("CARGO_MANIFEST_DIR"));
    let static_registry = load_documents(&schema_path)?;

    // 2. Start database container
    let (database, _container) = start_postgres(static_registry).await?;

    // 3. Initialize app state and Axum router
    let repository = PostgresDocumentsRepository::new(static_registry, database);
    let state = AppStateImpl::new(static_registry, repository, Default::default());

    let router = Router::new()
        .route("/health", get(health_check))
        .nest("/api", api_routes())
        .with_state(state);

    // --- TEST 1: Health Check (Simple 200 OK) ---
    {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), 1000).await?;
        assert!(body_bytes.is_empty());
    }

    // --- TEST 2: Create a Brand (Success) & GET it ---
    let brand_location;
    {
        let body_str = r#"{"data": {"uid": "brand-123", "name": "Brand One"}}"#.to_string();
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/documents/brands")
                    .header("content-type", "application/json")
                    .body(Body::from(body_str))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::CREATED);
        let loc = response.headers().get("location").unwrap().to_str().unwrap();
        brand_location = format!("{}?status=draft", loc);
    }

    // Now GET the created brand
    {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&brand_location)
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), 10000).await?;
        let json: Value = serde_json::from_slice(&body_bytes)?;
        assert!(json.get("data").is_some());
        assert_eq!(json.get("data").unwrap().get("uid").unwrap(), "brand-123");
    }

    // --- TEST 3: Create Duplicate Brand (409 Conflict with RFC 7807/9457 details) ---
    {
        let body_str = r#"{"data": {"uid": "brand-123", "name": "Brand Duplicate"}}"#.to_string();
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/documents/brands")
                    .header("content-type", "application/json")
                    .body(Body::from(body_str))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response.headers().get("content-type").unwrap(), "application/problem+json");

        let body_bytes = axum::body::to_bytes(response.into_body(), 10000).await?;
        let json: Value = serde_json::from_slice(&body_bytes)?;
        assert_eq!(json.get("type").unwrap(), "about:blank");
        assert_eq!(json.get("title").unwrap(), "Conflict");
        assert_eq!(json.get("status").unwrap(), 409);
        assert!(json.get("detail").unwrap().as_str().unwrap().contains("uid"));
    }

    // --- TEST 4: Create Partner with Non-Existent Relation (422 Unprocessable Entity with RFC 7807/9457 details) ---
    {
        let invalid_category_uuid = uuid::Uuid::new_v4().to_string();
        let body_str = format!(
            r#"{{"data": {{"idno": "1234567890123", "legal_entity": "Partner Rel LLC", "category": {{"connect": ["{}"]}}}}}}"#,
            invalid_category_uuid
        );
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/documents/partners")
                    .header("content-type", "application/json")
                    .body(Body::from(body_str))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(response.headers().get("content-type").unwrap(), "application/problem+json");

        let body_bytes = axum::body::to_bytes(response.into_body(), 10000).await?;
        let json: Value = serde_json::from_slice(&body_bytes)?;
        assert_eq!(json.get("type").unwrap(), "about:blank");
        assert_eq!(json.get("title").unwrap(), "Unprocessable Entity");
        assert_eq!(json.get("status").unwrap(), 422);
        let detail = json.get("detail").unwrap().as_str().unwrap();
        if !detail.contains("Relation constraint violation") {
            panic!("Expected relation constraint violation, but detail is: {}", detail);
        }
    }

    // --- TEST 5: Pagination limit (Capped at 100) ---
    {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/documents/brands?pagination[pageSize]=200")
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), 10000).await?;
        let json: Value = serde_json::from_slice(&body_bytes)?;
        // Check pagination pageSize in response metadata matches 100 (capped)
        let meta = json.get("meta").unwrap();
        let page_size = meta
            .get("pageSize")
            .or_else(|| meta.get("page_size"))
            .unwrap()
            .as_u64()
            .unwrap();
        assert_eq!(page_size, 100);
    }

    Ok(())
}
