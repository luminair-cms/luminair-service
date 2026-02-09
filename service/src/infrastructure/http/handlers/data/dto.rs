use chrono::{DateTime, Utc};
use luminair_common::AttributeId;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use std::{collections::HashMap, io::ErrorKind};

use crate::domain::document::{
    DocumentInstance, DocumentInstanceId,
    content::{ContentValue, DomainValue},
};

#[derive(Debug, Clone, Serialize)]
pub struct ManyDocumentsResponse {
    pub data: Vec<DocumentInstanceResponse>,
    pub meta: MetadataResponse,
}

impl From<Vec<DocumentInstance>> for ManyDocumentsResponse {
    fn from(value: Vec<DocumentInstance>) -> Self {
        let meta = MetadataResponse { total: value.len() };
        Self { data: value.into_iter().map(DocumentInstanceResponse::from).collect(), meta }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetadataResponse {
    pub total: usize,
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
            .map(|row| OneDocumentResponse { data: DocumentInstanceResponse::from(row) })
            .ok_or_else(|| std::io::Error::new(ErrorKind::NotFound, "Document not found"))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInstanceResponse {
    pub document_id: DocumentInstanceId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    fields: HashMap<String, AttributeResponse>,
}

impl DocumentInstanceResponse {
    pub fn with_relations(
        self,
        relations: HashMap<AttributeId, Vec<DocumentInstanceResponse>>,
    ) -> Self {
        let mut fields: HashMap<String, AttributeResponse> = relations
            .into_iter()
            .map(|(k, v)| (k.to_string(), AttributeResponse::Relation(v)))
            .collect();
        fields.extend(self.fields);
        Self { fields, ..self }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AttributeResponse {
    Field(JsonValue),
    LocalizedField(HashMap<String, String>),
    Relation(Vec<DocumentInstanceResponse>),
}

impl PartialEq for DocumentInstanceResponse {
    fn eq(&self, other: &Self) -> bool {
        self.document_id == other.document_id
    }
}

impl From<DomainValue> for JsonValue {
    fn from(value: DomainValue) -> Self {
        match value {
            DomainValue::Text(text) => JsonValue::String(text),
            DomainValue::Integer(num) => JsonValue::Number(num.into()),
            DomainValue::Decimal(num) => serde_json::Number::from_f64(num)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            DomainValue::Boolean(b) => JsonValue::Bool(b),
            DomainValue::Date(date) => JsonValue::String(date.to_string()),
            DomainValue::DateTime(dt) => JsonValue::String(dt.to_rfc3339()),
            DomainValue::Email(email) => JsonValue::String(email.as_str().to_string()),
            DomainValue::Url(url) => JsonValue::String(url.as_str().to_string()),
            DomainValue::Uuid(uuid) => JsonValue::String(uuid.to_string()),
            DomainValue::Json(json_blob) => json_blob.as_value().clone(),
            DomainValue::Null => JsonValue::Null,
        }
    }
}

impl From<DocumentInstance> for DocumentInstanceResponse {
    fn from(value: DocumentInstance) -> Self {
        let document_id = value.id.into();

        let audit = value.audit;
        let created_at = audit.created_at;
        let updated_at = audit.updated_at;
        let published_at = audit.published_at;

        let fields: HashMap<String, AttributeResponse> = value
            .content
            .fields
            .iter()
            .map(|(k, v)| {
                (
                    k.to_owned(),
                    match v {
                        ContentValue::Scalar(domain_value) => {
                            AttributeResponse::Field(JsonValue::from(domain_value.clone()))
                        }
                        ContentValue::LocalizedText(value) => {
                            AttributeResponse::LocalizedField(value.to_owned())
                        }
                    },
                )
            })
            .collect();

        Self {
            document_id,
            created_at,
            updated_at,
            published_at,
            fields,
        }
    }
}
/*
pub struct GroupedDocumentRowResponse {
    pub owning_id: DocumentRowId,
    pub rows: Vec<DocumentRowResponse>
}
 */
