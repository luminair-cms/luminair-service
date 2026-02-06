use crate::{
    domain::{AppState, HelloService},
    infrastructure::persistence::{PersistenceAdapter, repository::PostgresDocumentRepository},
};
use anyhow::anyhow;
use luminair_common::{DocumentTypesRegistry, documents::Documents};
use luminair_common::database::Database;

pub mod http;
pub mod persistence;
pub mod settings;

#[derive(Clone, Debug)]
pub struct HelloServiceAdapter {
    database: &'static Database,
}

impl HelloServiceAdapter {
    pub fn new(database: &'static Database) -> Self {
        Self { database }
    }
}

impl HelloService for HelloServiceAdapter {
    async fn hello(&self) -> Result<String, anyhow::Error> {
        sqlx::query_scalar("select 'hello world from pg'")
            .fetch_one(self.database.database_pool())
            .await
            .map_err(|e| anyhow!("failed to execute query: {}", e))
    }
}

#[derive(Clone)]
pub struct AppStateImpl {
    hello_service: HelloServiceAdapter,
    schema_registry: &'static dyn DocumentTypesRegistry,
    repository: PostgresDocumentRepository,
}

impl AppStateImpl {
    pub fn new(
        hello_service: HelloServiceAdapter,
        schema_registry: &'static dyn DocumentTypesRegistry,
        repository: PostgresDocumentRepository,
    ) -> Self {
        Self {
            hello_service,
            schema_registry,
            repository,
        }
    }
}

impl AppState for AppStateImpl {
    type H = HelloServiceAdapter;
    type S = DocumentTypesRegistry;
    type R = PostgresDocumentRepository;

    fn hello_service(&self) -> &Self::H {
        &self.hello
    }

    fn documents(&self) -> &'static dyn Documents {
        self.documents
    }

    fn persistence(&self) -> &Self::P {
        &self.persistence
    }
}
