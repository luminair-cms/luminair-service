//! Integration tests for the `migration` crate.
//!
//! Each test spins up a dedicated PostgreSQL container via `testcontainers-modules`,
//! so Docker must be running when executing these tests.
//!
//! Run with:
//! ```bash
//! cargo test --package migration --test migration_integration
//! ```

use luminair_common::{
    DocumentTypesRegistry,
    InMemoryDocumentTypesRegistry,
    entities::DocumentType,
};
use migration::{
    application::{Migration, Persistence},
    infrastructure::persistence::PersistenceAdapter,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

// ---------------------------------------------------------------------------
// Container / connection helpers
// ---------------------------------------------------------------------------

/// Boots a fresh Postgres container and returns the connected [`PgPool`] and
/// the container guard (must stay alive for the test duration).
async fn start_postgres() -> anyhow::Result<(PgPool, impl Drop)> {
    let container = Postgres::default()
        .start()
        .await?;

    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(5432).await?;

    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;

    Ok((pool, container))
}

/// Creates a uniquely named schema for one test run.
///
/// Returns the schema name so the test can pass it to migration and assertions.
async fn isolated_schema(pool: &PgPool) -> anyhow::Result<String> {
    let name = format!("test_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA \"{name}\"")))
        .execute(pool)
        .await?;
    Ok(name)
}

/// Helper to drop the isolated schema after test execution.
async fn drop_schema(pool: &PgPool, schema: &str) -> anyhow::Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE")))
        .execute(pool)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Document factory
// ---------------------------------------------------------------------------

/// Builds the minimal [`DocumentType`] needed for schema generation.
fn make_document(name: &str) -> DocumentType {
    DocumentType::new_bare_collection(name, name, &format!("{name}s"))
        .unwrap_or_else(|e| panic!("make_document({name}): {e}"))
}

// ---------------------------------------------------------------------------
// Migration execution helper
// ---------------------------------------------------------------------------

/// Runs one migration pass against `pool` / `schema` with the given document list.
///
/// Returns the `PersistenceAdapter` so callers can call `persistence.load()` for
/// assertions — keeping tests at the application port level.
async fn run_migration(
    pool: &PgPool,
    schema: &str,
    docs: Vec<DocumentType>,
) -> anyhow::Result<PersistenceAdapter> {
    let registry = InMemoryDocumentTypesRegistry::from_vec(docs);
    let static_registry: &'static dyn DocumentTypesRegistry = Box::leak(Box::new(registry));

    let persistence = PersistenceAdapter::new(pool.clone(), schema);
    Migration::new(static_registry, persistence.clone())
        .migrate(false)
        .await?;

    Ok(persistence)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verifies that a PostgreSQL container can be started and that a live
/// connection can be established.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_connect_to_database() -> anyhow::Result<()> {
    let (pool, _container) = start_postgres().await?;

    let (value,): (i32,) = sqlx::query_as("SELECT 1")
        .fetch_one(&pool)
        .await?;

    assert_eq!(value, 1, "expected scalar 1 from the database");
    Ok(())
}

/// Verifies that running a migration against an empty database creates every
/// table required by the document registry.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_create_tables_on_fresh_database() -> anyhow::Result<()> {
    let (pool, _container) = start_postgres().await?;
    let schema = isolated_schema(&pool).await?;

    let persistence = run_migration(&pool, &schema, vec![
        make_document("alpha"),
        make_document("beta"),
    ]).await?;

    let actual = persistence.load().await?;
    let names: Vec<&str> = actual.iter().map(|t| t.name.as_str()).collect();

    assert!(
        names.iter().any(|&n| n == "alpha"),
        "table 'alpha' must exist after migration; got: {names:?}"
    );
    assert!(
        names.iter().any(|&n| n == "beta"),
        "table 'beta' must exist after migration; got: {names:?}"
    );

    drop_schema(&pool, &schema).await?;
    Ok(())
}

/// Verifies that a second migration pass creates only the newly introduced
/// table while leaving previously created tables untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_create_additional_table() -> anyhow::Result<()> {
    let (pool, _container) = start_postgres().await?;
    let schema = isolated_schema(&pool).await?;

    // --- First pass: only 'gamma' ---
    let persistence_v1 = run_migration(&pool, &schema, vec![make_document("gamma")]).await?;

    let tables_v1 = persistence_v1.load().await?;
    let names_v1: Vec<&str> = tables_v1.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names_v1.iter().any(|&n| n == "gamma"),
        "table 'gamma' must exist after first pass; got: {names_v1:?}"
    );
    assert!(
        !names_v1.iter().any(|&n| n == "delta"),
        "table 'delta' must NOT exist yet; got: {names_v1:?}"
    );

    // --- Second pass: add 'delta' alongside 'gamma' ---
    let persistence_v2 = run_migration(&pool, &schema, vec![
        make_document("gamma"),
        make_document("delta"),
    ]).await?;

    let tables_v2 = persistence_v2.load().await?;
    let names_v2: Vec<&str> = tables_v2.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names_v2.iter().any(|&n| n == "gamma"),
        "table 'gamma' must still exist after second pass; got: {names_v2:?}"
    );
    assert!(
        names_v2.iter().any(|&n| n == "delta"),
        "table 'delta' must be created by second pass; got: {names_v2:?}"
    );

    drop_schema(&pool, &schema).await?;
    Ok(())
}

/// Verifies that a document type removed from the registry causes the
/// corresponding database table to be dropped on the next migration run.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_remove_obsolete_table() -> anyhow::Result<()> {
    let (pool, _container) = start_postgres().await?;
    let schema = isolated_schema(&pool).await?;

    // --- First pass: create 'epsilon' and 'zeta' ---
    let persistence_v1 = run_migration(&pool, &schema, vec![
        make_document("epsilon"),
        make_document("zeta"),
    ]).await?;

    let tables_v1 = persistence_v1.load().await?;
    let names_v1: Vec<&str> = tables_v1.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names_v1.iter().any(|&n| n == "epsilon"),
        "table 'epsilon' must exist; got: {names_v1:?}"
    );
    assert!(
        names_v1.iter().any(|&n| n == "zeta"),
        "table 'zeta' must exist; got: {names_v1:?}"
    );

    // --- Second pass: remove 'zeta' from the registry ---
    let persistence_v2 = run_migration(&pool, &schema, vec![make_document("epsilon")]).await?;

    let tables_v2 = persistence_v2.load().await?;
    let names_v2: Vec<&str> = tables_v2.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names_v2.iter().any(|&n| n == "epsilon"),
        "table 'epsilon' must still exist; got: {names_v2:?}"
    );
    assert!(
        !names_v2.iter().any(|&n| n == "zeta"),
        "table 'zeta' must have been dropped; got: {names_v2:?}"
    );

    drop_schema(&pool, &schema).await?;
    Ok(())
}
