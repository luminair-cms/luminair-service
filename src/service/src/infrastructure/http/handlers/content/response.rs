use crate::domain::document::lifecycle::PublicationState;
use crate::domain::document::{DocumentInstance, content::ContentValue};
use chrono::{DateTime, Utc};
use luminair_common::AttributeId;

use serde::Serialize;
use serde_json::Value as JsonValue;
use std::{collections::HashMap, io::ErrorKind};

#[derive(Debug, Clone, Serialize)]
pub struct ManyDocumentsResponse {
    pub data: Vec<DocumentInstanceResponse>,
    pub meta: MetadataResponse,
}

impl ManyDocumentsResponse {
    pub fn new(documents: Vec<DocumentInstance>, page: u16, page_size: u16, total: u64) -> Self {
        let meta = MetadataResponse { page, page_size, total };
        Self {
            data: documents
                .into_iter()
                .map(DocumentInstanceResponse::from)
                .collect(),
            meta,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetadataResponse {
    pub page: u16,
    pub page_size: u16,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OneDocumentResponse {
    pub data: DocumentInstanceResponse,
}

impl PartialEq for OneDocumentResponse {
    fn eq(&self, other: &Self) -> bool {
        self.data.document_id == other.data.document_id
    }
}

impl TryFrom<Option<DocumentInstance>> for OneDocumentResponse {
    type Error = std::io::Error;

    fn try_from(value: Option<DocumentInstance>) -> Result<Self, Self::Error> {
        value
            .map(|row| OneDocumentResponse {
                data: DocumentInstanceResponse::from(row),
            })
            .ok_or_else(|| std::io::Error::new(ErrorKind::NotFound, "Document not found"))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInstanceResponse {
    pub id: i64,
    pub document_id: String,
    #[serde(flatten)]
    pub audit: DocumentInstanceAudit,
    #[serde(flatten)]
    pub published: Option<DocumentInstancePublicationState>,
    #[serde(flatten)]
    fields: HashMap<String, AttributeResponse>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInstanceAudit {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInstancePublicationState {
    pub published_at: DateTime<Utc>,
    pub published_by: Option<String>,
    pub revision: i32,
}



#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AttributeResponse {
    Field(JsonValue),
    Relation(Vec<DocumentInstanceResponse>),
}

impl PartialEq for DocumentInstanceResponse {
    fn eq(&self, other: &Self) -> bool {
        self.document_id == other.document_id
    }
}

impl From<DocumentInstance> for DocumentInstanceResponse {
    fn from(value: DocumentInstance) -> Self {
        let id = value.id.0;
        let document_id = value.document_id.into();

        let audit = value.audit;
        let created_at = audit.created_at;
        let updated_at = audit.updated_at;

        let audit = DocumentInstanceAudit {
            created_at,
            updated_at,
            created_by: audit.created_by.map(|u| u.into()),
            updated_by: audit.updated_by.map(|u| u.into()),
            version: audit.version,
        };

        let published = match value.content.publication_state {
            PublicationState::Draft { revision: _ } => None,
            PublicationState::Published {
                revision,
                published_at,
                published_by,
            } => Some(DocumentInstancePublicationState {
                revision,
                published_at,
                published_by: published_by.map(|u| u.into()),
            }),
        };

        // ContentValue → JsonValue is handled by the domain codec (From<&ContentValue>).
        let mut fields: HashMap<String, AttributeResponse> = value
            .content
            .fields
            .iter()
            .map(|(k, v)| {
                let json_value = JsonValue::from(v);
                (
                    to_api_key(k.as_ref()),
                    AttributeResponse::Field(json_value),
                )
            })
            .collect();

        for (rel_attr, rel_list) in value.relations {
            let rel_responses: Vec<DocumentInstanceResponse> = rel_list
                .into_iter()
                .filter_map(|r| match r {
                    crate::domain::document::DocumentRelation::Instance(inst) => {
                        Some(DocumentInstanceResponse::from(inst))
                    }
                    crate::domain::document::DocumentRelation::Id(_) => None,
                })
                .collect();
            if !rel_responses.is_empty() {
                fields.insert(
                    to_api_key(rel_attr.as_ref()),
                    AttributeResponse::Relation(rel_responses),
                );
            }
        }

        Self {
            id,
            document_id,
            audit,
            published,
            fields,
        }
    }
}

fn to_api_key(snake: &str) -> String {
    // "first_name" → "firstName"
    let mut result = String::with_capacity(snake.len());
    let mut next_upper = false;
    for c in snake.chars() {
        if c == '_' { next_upper = true; }
        else if next_upper { result.extend(c.to_uppercase()); next_upper = false; }
        else { result.push(c); }
    }
    result
}