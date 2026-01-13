use std::collections::HashMap;
use chrono::{DateTime, Utc};
use luminair_common::domain::Documents;

use crate::domain::query::Query;

pub mod query;

/// This trait used only for testing purposes.
pub trait HelloService: Send + Sync + 'static {
    fn hello(&self) -> impl Future<Output = Result<String, anyhow::Error>> + Send;
}

/// Service that translate requests to a document model into requests to db
/// and provide serialize/deserialize
pub trait Persistence: Clone + Send + Sync + 'static {
    /// select rows from a database
    fn select_all(
        &self,
        query: Query<'_>,
    ) -> impl Future<Output = Result<impl ResultSet, anyhow::Error>> + Send;
    
    /// select rows from a database
    fn select_by_id(
        &self,
        query: Query<'_>,
        id: i32
    ) -> impl Future<Output = Result<impl ResultSet, anyhow::Error>> + Send;
}

pub trait ResultSet: Send {
    fn into_rows(self) -> Vec<ResultRow>;
}

pub struct ResultRow {
    pub document_id: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub locale: Option<String>,
    pub fields: HashMap<String,String>,
    pub localized_fields: HashMap<String,String>
}

//// The global application state shared between all request handlers.
pub trait AppState: Clone + Send + Sync + 'static {
    type H: HelloService;
    type P: Persistence;
    fn hello_service(&self) -> &Self::H;
    fn documents(&self) -> &'static dyn Documents;
    fn persistence(&self) -> &Self::P;
}
