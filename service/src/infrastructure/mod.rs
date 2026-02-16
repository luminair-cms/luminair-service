use crate::{
    domain::{AppState, HelloService},
    infrastructure::persistence::repository::PostgresDocumentRepository,
};
use anyhow::anyhow;
use luminair_common::DocumentTypesRegistry;
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
    document_types_registry: &'static dyn DocumentTypesRegistry,
    document_type_index: crate::domain::DocumentTypeIndex,
    documents_instance_repository: PostgresDocumentRepository,
}

impl AppStateImpl {
    pub fn new(
        hello_service: HelloServiceAdapter,
        document_types_registry: &'static dyn DocumentTypesRegistry,
        documents_instance_repository: PostgresDocumentRepository,
    ) -> Self {
        let document_type_index = crate::domain::DocumentTypeIndex::new(document_types_registry);
        Self {
            hello_service,
            document_types_registry,
            document_type_index,
            documents_instance_repository,
        }
    }
}

impl AppState for AppStateImpl {
    type H = HelloServiceAdapter;
    type R = PostgresDocumentRepository;

    fn hello_service(&self) -> &Self::H {
        &self.hello_service
    }

    fn document_types_registry(&self) -> &'static dyn DocumentTypesRegistry {
        self.document_types_registry
    }

    fn document_type_index(&self) -> &crate::domain::DocumentTypeIndex {
        &self.document_type_index
    }

    fn documents_instance_repository(&self) -> &Self::R {
        &self.documents_instance_repository
    }
}
