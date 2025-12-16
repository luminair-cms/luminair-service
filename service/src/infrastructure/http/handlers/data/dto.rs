use std::collections::HashMap;
use serde::Serialize;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize)]
pub struct ManyDocumentRowsResponse {
    data: Vec<DocumentRowResponse>,
    meta: MetadataResponse
}

impl PartialEq for ManyDocumentRowsResponse {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetadataResponse {
    total: usize
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
    id: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    body: HashMap<String,String>
}

impl PartialEq for DocumentRowResponse {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}