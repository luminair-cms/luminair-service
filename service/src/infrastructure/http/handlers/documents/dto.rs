use luminair_common::domain::attributes::{
    Attribute, AttributeBody, AttributeConstraints, AttributeType, RelationType,
};
use luminair_common::domain::documents::{Document, DocumentInfo, DocumentOptions, DocumentType};
use serde::Serialize;

/// Response for list documents route
#[derive(Debug, Clone, Serialize)]
pub struct DocumentResponse {
    id: String,
    title: String,
    document_type: DocumentType,
    description: String,
}

impl PartialEq for DocumentResponse {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl From<&Document> for DocumentResponse {
    fn from(value: &Document) -> Self {
        Self {
            id: value.id.as_ref().to_string(),
            title: value.info.title.as_ref().to_string(),
            document_type: value.document_type.clone(),
            description: value.info.description.as_ref().to_string(),
        }
    }
}

/// Response for one document route
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedDocumentResponse {
    id: String,
    title: String,
    document_type: DocumentType,
    info: DocumentInfoResponse,
    options: Option<DocumentOptionsResponse>,
    attributes: Vec<AttributeResponse>,
}

/// Document info from one document response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfoResponse {
    title: String,
    description: String,
    singular_name: String,
    plural_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentOptionsResponse {
    pub draft_and_publish: bool,
    pub localizations: Vec<String>,
}

/// Attribute of Document resonse
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeResponse {
    id: String,
    #[serde(flatten)]
    body: AttribteBodyResponse,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum AttribteBodyResponse {
    Field {
        attribute_type: AttributeType,
        unique: bool,
        #[serde(default)]
        required: bool,
        #[serde(default)]
        localized: bool,
        constraints: Option<AttributeConstraints>,
    },
    Relation {
        #[serde(alias = "relation")]
        relation_type: RelationType,
        target: String,
        #[serde(default)]
        ordering: bool,
    },
}

impl PartialEq for DetailedDocumentResponse {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl From<&Document> for DetailedDocumentResponse {
    fn from(value: &Document) -> Self {
        Self {
            id: value.id.to_string(),
            title: value.info.title.to_string(),
            document_type: value.document_type.clone(),
            info: (&value.info).into(),
            options: value.options.as_ref().map(DocumentOptionsResponse::from),
            attributes: value
                .attributes
                .iter()
                .map(AttributeResponse::from)
                .collect(),
        }
    }
}

impl From<&DocumentInfo> for DocumentInfoResponse {
    fn from(value: &DocumentInfo) -> Self {
        Self {
            title: value.title.to_string(),
            description: value.description.to_string(),
            singular_name: value.singular_name.to_string(),
            plural_name: value.plural_name.to_string(),
        }
    }
}

impl From<&DocumentOptions> for DocumentOptionsResponse {
    fn from(value: &DocumentOptions) -> Self {
        Self {
            draft_and_publish: value.draft_and_publish,
            localizations: value.localizations.iter().map(|l| l.to_string()).collect(),
        }
    }
}

impl From<&Attribute> for AttributeResponse {
    fn from(value: &Attribute) -> Self {
        let id = value.id.to_string();
        let body = match &value.body {
            AttributeBody::Field {
                attribute_type,
                unique,
                required,
                localized,
                constraints,
            } => {
                let constraints = constraints.as_ref().map(|c| c.clone());
                AttribteBodyResponse::Field {
                    attribute_type: attribute_type.clone(),
                    unique: *unique,
                    required: *required,
                    localized: *localized,
                    constraints,
                }
            }
            AttributeBody::Relation {
                relation_type,
                target,
                ordering,
            } => {
                let target = target.to_string();
                AttribteBodyResponse::Relation {
                    relation_type: relation_type.clone(),
                    target,
                    ordering: *ordering,
                }
            }
        };

        Self { id, body }
    }
}
