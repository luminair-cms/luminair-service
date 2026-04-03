use crate::{
    domain::{AppState},
};
use anyhow::anyhow;
use luminair_common::DocumentTypesRegistry;
use luminair_common::database::Database;
use crate::domain::application::implementation::DocumentServicesImpl;
use crate::infrastructure::persistence::repository::PostgresDocumentsRepository;

pub mod http;
pub mod persistence;
pub mod settings;

#[derive(Clone)]
pub struct AppStateImpl {
    types: &'static dyn DocumentTypesRegistry,
    documents_services: DocumentServicesImpl<PostgresDocumentsRepository>,
}

impl AppStateImpl {
    pub fn new(
        types: &'static dyn DocumentTypesRegistry,
        documents_repository: PostgresDocumentsRepository,
    ) -> Self {
        Self {
            types,
            documents_services: DocumentServicesImpl::new(documents_repository, types),
        }
    }
}

impl AppState for AppStateImpl {
    type D = DocumentServicesImpl<PostgresDocumentsRepository>;

    fn document_types(&self) -> &'static dyn DocumentTypesRegistry {
        self.types
    }

    fn documents_services(&self) -> &Self::D {
        &self.documents_services
    }
}
