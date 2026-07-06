//! End-to-end service integration tests.
//!
//! These tests exercise the full stack — HTTP routing, application service layer,
//! and PostgreSQL persistence — using a real containerised database provisioned by
//! `testcontainers`. They are intentionally kept at a higher level than unit tests
//! and serve as the primary regression guard for cross-layer behaviour.
//!
//! # Registry initialisation
//!
//! `load_documents` stores the parsed schema in a process-wide `OnceLock`, so it
//! must be called **exactly once** per test binary. We guard this with a local
//! `OnceLock` and expose a `registry()` helper that all tests use.

use std::sync::OnceLock;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use serde_json::Value;
use service::{
    infrastructure::{
        AppStateImpl,
        http::{handlers::health_check, routes::api_routes},
        persistence::repository::PostgresDocumentsRepository,
    },
};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
use tower::ServiceExt;

use luminair_common::{
    database::{self, DatabaseConnection, DatabaseCredentials, DatabaseSettings},
    load_documents, DocumentTypesRegistry,
};
use migration::{application::Migration, infrastructure::persistence::PersistenceAdapter};

// ---------------------------------------------------------------------------
// Registry — initialised once per test binary
// ---------------------------------------------------------------------------

static REGISTRY: OnceLock<&'static dyn DocumentTypesRegistry> = OnceLock::new();

fn registry() -> &'static dyn DocumentTypesRegistry {
    *REGISTRY.get_or_init(|| {
        let schema_path = format!("{}/../../config/schema", env!("CARGO_MANIFEST_DIR"));
        load_documents(&schema_path).expect("failed to load schema registry")
    })
}

// ---------------------------------------------------------------------------
// Test harness helpers
// ---------------------------------------------------------------------------

/// Boot a fresh Postgres container, apply migrations, and return the pool + guard.
async fn start_postgres() -> anyhow::Result<(&'static database::Database, impl Drop)> {
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

    let database = database::connect(&settings).await?;
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

type TestRouter = Router;

/// Build a fully wired Axum router backed by a fresh isolated database.
async fn build_router() -> anyhow::Result<(TestRouter, impl Drop)> {
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

async fn get_json(router: &TestRouter, uri: &str) -> anyhow::Result<(StatusCode, Value)> {
    let response = router
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty())?)
        .await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await?;
    let json = serde_json::from_slice(&bytes)?;
    Ok((status, json))
}

async fn post_json(
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

async fn put_json(
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
async fn create_document(
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

/// POST to `{document_uri}/publish`; returns the published document JSON.
async fn publish_document(router: &TestRouter, document_uri: &str) -> anyhow::Result<Value> {
    let (status, _, bytes) = post_json(router, &format!("{document_uri}/publish"), "{}").await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "publish failed: {}",
        String::from_utf8_lossy(&bytes)
    );
    Ok(serde_json::from_slice(&bytes)?)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

async fn create_brand(router: &TestRouter, uid: &str, name: &str) -> anyhow::Result<String> {
    create_document(
        router,
        "brands",
        &format!(r#"{{"data": {{"uid": "{uid}", "name": "{name}"}}}}"#),
    )
    .await
}

async fn create_partner(
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

async fn create_partner_category(
    router: &TestRouter,
    uid: &str,
    priority: i32,
) -> anyhow::Result<String> {
    create_document(
        router,
        "partner-categories",
        &format!(
            r#"{{"data": {{"uid": "{uid}", "name": "Category {uid}", "priority": {priority}}}}}"#
        ),
    )
    .await
}

// ---------------------------------------------------------------------------
// Tests — health check
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_check_returns_200_empty_body() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let response = router
        .oneshot(Request::builder().uri("/health").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1000).await?;
    assert!(bytes.is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_and_retrieve_draft_document() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let loc = create_brand(&router, "brand-a", "Alpha Brand").await?;

    let (status, json) = get_json(&router, &format!("{loc}?status=draft")).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["uid"], "brand-a");
    assert_eq!(json["data"]["name"], "Alpha Brand");
    Ok(())
}

#[tokio::test]
async fn duplicate_unique_field_returns_409_problem_details() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    create_brand(&router, "dup-uid", "Original").await?;

    let (status, _, bytes) = post_json(
        &router,
        "/api/documents/brands",
        r#"{"data": {"uid": "dup-uid", "name": "Duplicate"}}"#,
    )
    .await?;
    let json: Value = serde_json::from_slice(&bytes)?;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["type"], "about:blank");
    assert_eq!(json["status"], 409);
    Ok(())
}

#[tokio::test]
async fn nonexistent_relation_target_returns_422_problem_details() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let phantom_id = uuid::Uuid::new_v4().to_string();
    let body = format!(
        r#"{{"data": {{"idno": "9000000000001", "legal_entity": "Ghost LLC", "category": {{"connect": ["{phantom_id}"]}}}}}}"#
    );
    let (status, _, bytes) = post_json(&router, "/api/documents/partners", &body).await?;
    let json: Value = serde_json::from_slice(&bytes)?;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["type"], "about:blank");
    assert_eq!(json["status"], 422);
    assert!(
        json["detail"]
            .as_str()
            .unwrap()
            .contains("Relation constraint violation"),
        "unexpected detail: {}",
        json["detail"]
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn filter_by_field_value() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    create_brand(&router, "fil-aaa", "Acme").await?;
    create_brand(&router, "fil-bbb", "Beta").await?;

    let (status, json) = get_json(
        &router,
        "/api/documents/brands?status=draft&filters[uid][$eq]=fil-aaa",
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let items = json["data"].as_array().expect("data must be an array");
    assert_eq!(items.len(), 1, "filter should return exactly one brand");
    assert_eq!(items[0]["uid"], "fil-aaa");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — sort / order
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sort_documents_by_field_ascending() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    create_brand(&router, "srt-zzz", "Zebra").await?;
    create_brand(&router, "srt-aaa", "Apple").await?;
    create_brand(&router, "srt-mmm", "Mango").await?;

    let (status, json) = get_json(
        &router,
        "/api/documents/brands?status=draft&sort=uid:asc",
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let items = json["data"].as_array().expect("data must be an array");
    assert!(items.len() >= 3, "expected at least 3 brands");

    let uids: Vec<&str> = items
        .iter()
        .map(|v| v["uid"].as_str().expect("uid field"))
        .collect();
    let mut sorted = uids.clone();
    sorted.sort();
    assert_eq!(uids, sorted, "brands should be in ascending uid order");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — pagination cap
// ---------------------------------------------------------------------------

#[tokio::test]
async fn page_size_is_capped_at_configured_maximum() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let (status, json) = get_json(
        &router,
        "/api/documents/brands?pagination[pageSize]=999",
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let meta = &json["meta"];
    let page_size = meta
        .get("pageSize")
        .or_else(|| meta.get("page_size"))
        .and_then(|v| v.as_u64())
        .expect("pageSize must be present in meta");
    assert_eq!(page_size, 100, "pageSize must be capped at 100");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — populate (relation loading)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn populate_loads_related_documents() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let cat_loc = create_partner_category(&router, "pop-retail", 1).await?;
    let cat_id = cat_loc
        .trim_start_matches("/api/documents/partner-categories/");

    let partner_loc =
        create_partner(&router, "5000000000001", "Populated Partner Ltd").await?;
    let partner_id = partner_loc.trim_start_matches("/api/documents/partners/");

    // Connect the category to the partner
    put_json(
        &router,
        &format!("/api/documents/partners/{partner_id}"),
        &format!(r#"{{"data": {{"category": {{"connect": ["{cat_id}"]}}}}}}"#),
    )
    .await?;

    // Fetch with ?populate=category
    let (status, json) = get_json(
        &router,
        &format!("/api/documents/partners/{partner_id}?status=draft&populate=category"),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let category = &json["data"]["category"];
    assert!(
        category.as_array().map(|a| !a.is_empty()).unwrap_or(false),
        "category relation should be populated, got: {category}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — connect / disconnect
// ---------------------------------------------------------------------------

#[tokio::test]
async fn connect_and_disconnect_relation() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let cat_loc = create_partner_category(&router, "con-retail", 2).await?;
    let cat_id = cat_loc
        .trim_start_matches("/api/documents/partner-categories/");

    let partner_loc =
        create_partner(&router, "6000000000001", "Connect Test Ltd").await?;
    let partner_id = partner_loc.trim_start_matches("/api/documents/partners/");

    // --- Connect ---
    let (status, _) = put_json(
        &router,
        &format!("/api/documents/partners/{partner_id}"),
        &format!(r#"{{"data": {{"category": {{"connect": ["{cat_id}"]}}}}}}"#),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "connect should return 200");

    let (_, json) = get_json(
        &router,
        &format!("/api/documents/partners/{partner_id}?status=draft&populate=category"),
    )
    .await?;
    assert!(
        json["data"]["category"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "category should be connected"
    );

    // --- Disconnect ---
    let (status, _) = put_json(
        &router,
        &format!("/api/documents/partners/{partner_id}"),
        &format!(r#"{{"data": {{"category": {{"disconnect": ["{cat_id}"]}}}}}}"#),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "disconnect should return 200");

    let (_, json) = get_json(
        &router,
        &format!("/api/documents/partners/{partner_id}?status=draft&populate=category"),
    )
    .await?;
    assert!(
        json["data"]["category"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(true),
        "category should be disconnected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — publish
// ---------------------------------------------------------------------------

#[tokio::test]
async fn publish_draft_makes_document_visible_as_published() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let loc = create_brand(&router, "pub-brd", "Published Brand").await?;

    // Draft exists
    let (status, _) = get_json(&router, &format!("{loc}?status=draft")).await?;
    assert_eq!(status, StatusCode::OK, "draft must be accessible");

    // Not published yet
    let (status, _) = get_json(&router, &loc).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "should not be published yet");

    // Publish
    let published = publish_document(&router, &loc).await?;
    assert_eq!(published["data"]["uid"], "pub-brd");

    // Now available as published (default status)
    let (status, json) = get_json(&router, &loc).await?;
    assert_eq!(status, StatusCode::OK, "must be accessible after publish");
    assert_eq!(json["data"]["uid"], "pub-brd");
    Ok(())
}

#[tokio::test]
async fn draft_copy_still_accessible_after_publish() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let loc = create_brand(&router, "pub-drft", "Still Draft").await?;
    publish_document(&router, &loc).await?;

    // Draft copy must still exist
    let (status, _) = get_json(&router, &format!("{loc}?status=draft")).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "draft copy should still be accessible after publish"
    );

    // Published copy must also exist
    let (status, _) = get_json(&router, &loc).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "published copy must be accessible"
    );
    Ok(())
}
