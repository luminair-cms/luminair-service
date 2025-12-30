use std::collections::HashMap;
use serde::Serialize;
use chrono::{DateTime, Utc};

use crate::domain::ResultRow;

#[derive(Debug, Clone, Serialize)]
pub struct ManyDocumentRowsResponse {
    pub data: Vec<DocumentRowResponse>,
    pub meta: MetadataResponse
}

impl PartialEq for ManyDocumentRowsResponse {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetadataResponse {
    pub total: usize
}

#[derive(Debug, Clone, Serialize)]
pub struct OneDocumentRowResponse {
    data: DocumentRowResponse
}

impl PartialEq for OneDocumentRowResponse {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRowResponse {
    document_id: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
    locale: Option<String>,
    #[serde(flatten)]
    body: HashMap<String,String>
}

impl PartialEq for DocumentRowResponse {
    fn eq(&self, other: &Self) -> bool {
        self.document_id == other.document_id
    }
}

impl From<ResultRow> for DocumentRowResponse {
    fn from(value: ResultRow) -> Self {
        Self {
            document_id: value.document_id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            published_at: value.published_at,
            locale: value.locale,
            body: value.body
        }
    }
}