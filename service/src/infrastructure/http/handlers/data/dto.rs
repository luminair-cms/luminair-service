use std::{collections::HashMap, io::ErrorKind};
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
    document_id: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    fields: HashMap<String,AttributeResponse>
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AttributeResponse {
    Field(String),
    LocalizedField(HashMap<String,String>)
}

impl PartialEq for DocumentRowResponse {
    fn eq(&self, other: &Self) -> bool {
        self.document_id == other.document_id
    }
}

impl From<(i32, Vec<ResultRow>)> for DocumentRowResponse {
    fn from((document_id, rows): (i32, Vec<ResultRow>)) -> Self {
        // safety: there must be at least one row for each document_id
        // many rows in case of localized fields: one row for each locale
        let value = unsafe { rows.get_unchecked(0) };
        
        let created_at = value.created_at;
        let updated_at = value.updated_at;
        let published_at = value.published_at;
        let mut fields: HashMap<String, AttributeResponse> = value.fields.iter()
            .map(|(k,v)|(k.to_owned(), AttributeResponse::Field(v.to_owned())))
            .collect();
        
        for row in rows.into_iter() {
            if let Some(ref locale) = row.locale {
                for (k,v) in row.localized_fields {
                    fields.entry(k)
                        .and_modify(|e| { 
                            match e {
                                AttributeResponse::LocalizedField(map) => {
                                    map.insert(locale.to_owned(), v.clone());
                                },
                                _ => unreachable!()
                            };
                        })
                        .or_insert_with(||AttributeResponse::LocalizedField(HashMap::from([(locale.to_owned(), v)])));
                }
            }
        }
        
        Self {
            document_id,
            created_at,
            updated_at,
            published_at,
            fields
        }
    }
}