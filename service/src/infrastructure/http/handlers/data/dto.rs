use std::{collections::HashMap, io::ErrorKind};
use serde::Serialize;
use chrono::{DateTime, Utc};

use crate::domain::{FieldValue, ResultRow};

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

impl From<Vec<DocumentRowResponse>> for ManyDocumentRowsResponse {
    fn from(value: Vec<DocumentRowResponse>) -> Self {
        let meta = MetadataResponse { total: value.len() };
        Self {
            data: value,
            meta
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetadataResponse {
    pub total: usize
}

#[derive(Debug, Clone, Serialize)]
pub struct OneDocumentRowResponse {
    pub data: DocumentRowResponse
}

impl PartialEq for OneDocumentRowResponse {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl TryFrom<Vec<DocumentRowResponse>> for OneDocumentRowResponse {
   type Error = std::io::Error;

    fn try_from(value: Vec<DocumentRowResponse>) -> Result<Self, Self::Error> {
        value.into_iter()
            .next()
            .map(|row| OneDocumentRowResponse { data: row })
            .ok_or_else(||std::io::Error::new(ErrorKind::NotFound, "Document not found"))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRowResponse {
    #[serde(skip_serializing)]
    pub owning_id: Option<i32>,
    pub document_id: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    fields: HashMap<String,AttributeResponse>
}

impl DocumentRowResponse {
    pub fn add_relation(&mut self, attribute_id: String, relation: Vec<DocumentRowResponse>) {
        self.fields.insert(attribute_id, AttributeResponse::Relation(relation));
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AttributeResponse {
    Field(String),
    LocalizedField(HashMap<String,String>),
    Relation(Vec<DocumentRowResponse>)
}

impl PartialEq for DocumentRowResponse {
    fn eq(&self, other: &Self) -> bool {
        self.document_id == other.document_id
    }
}

impl From<ResultRow> for DocumentRowResponse {
    fn from(value: ResultRow) -> Self {
        let owning_id = value.owning_id;
        let document_id = value.document_id;
        let created_at = value.created_at;
        let updated_at = value.updated_at;
        let published_at = value.published_at;
        
        let fields: HashMap<String, AttributeResponse> = value.fields.iter()
            .map(|(k,v)|(k.to_owned(), match v {
                FieldValue::Ordinal(value) => AttributeResponse::Field(value.to_owned()),
                FieldValue::Localized(value) => AttributeResponse::LocalizedField(value.to_owned())
            }))
            .collect();
        
        Self {
            owning_id,
            document_id,
            created_at,
            updated_at,
            published_at,
            fields
        }
    }
}