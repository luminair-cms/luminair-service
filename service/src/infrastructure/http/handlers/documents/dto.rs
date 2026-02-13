use luminair_common::{
    AttributeId, DocumentType,
    entities::{
        AttributeConstraints, AttributeType, DocumentField, DocumentKind, DocumentRelation,
        DocumentTypeInfo, DocumentTypeOptions, RelationType,
    },
};
use serde::Serialize;

/// Response for list documents route
#[derive(Debug, Clone, Serialize)]
pub struct DocumentResponse {
    id: String,
    title: String,
    #[serde(alias = "type")]
    kind: DocumentKind,
    description: Option<String>,
}

impl PartialEq for DocumentResponse {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl From<&DocumentType> for DocumentResponse {
    fn from(value: &DocumentType) -> Self {
        Self {
            id: value.id.as_ref().to_string(),
            title: value.info.title.as_ref().to_string(),
            kind: value.kind.clone(),
            description: value.info.description.clone(),
        }
    }
}

/// Response for one document route
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedDocumentResponse {
    id: String,
    title: String,
    #[serde(alias = "type")]
    kind: DocumentKind,
    info: DocumentInfoResponse,
    options: Option<DocumentOptionsResponse>,
    attributes: Vec<AttributeResponse>,
}

/// Document info from one document response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfoResponse {
    title: String,
    description: Option<String>,
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
    },
}

impl PartialEq for DetailedDocumentResponse {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl From<&DocumentType> for DetailedDocumentResponse {
    fn from(value: &DocumentType) -> Self {
        let mut attributes = Vec::with_capacity(value.fields.len() + value.relations.len());

        for f in value.fields.iter() {
            attributes.push(f.into())
        }

        for r in value.relations.iter() {
            attributes.push(r.into());
        }

        Self {
            id: value.id.to_string(),
            title: value.info.title.to_string(),
            kind: value.kind.clone(),
            info: (&value.info).into(),
            options: value.options.as_ref().map(DocumentOptionsResponse::from),
            attributes,
        }
    }
}

impl From<&DocumentTypeInfo> for DocumentInfoResponse {
    fn from(value: &DocumentTypeInfo) -> Self {
        Self {
            title: value.title.to_string(),
            description: value.description.clone(),
            singular_name: value.singular_name.to_string(),
            plural_name: value.plural_name.to_string(),
        }
    }
}

impl From<&DocumentTypeOptions> for DocumentOptionsResponse {
    fn from(value: &DocumentTypeOptions) -> Self {
        Self {
            draft_and_publish: value.draft_and_publish,
            localizations: value.localizations.iter().map(|l| l.to_string()).collect(),
        }
    }
}

impl From<(&AttributeId, &DocumentField)> for AttributeResponse {
    fn from(value: (&AttributeId, &DocumentField)) -> Self {
        let id = value.0.to_string();
        let body = value.1;
        let constraints = body.constraints.as_ref().map(|c| c.clone());
        let body = AttribteBodyResponse::Field {
            attribute_type: body.attribute_type.clone(),
            unique: body.unique,
            required: body.required,
            localized: body.localized,
            constraints,
        };
        Self { id, body }
    }
}

impl From<(&AttributeId, &DocumentRelation)> for AttributeResponse {
    fn from(value: (&AttributeId, &DocumentRelation)) -> Self {
        let id = value.0.to_string();
        let body = value.1;
        let target = body.target.to_string();
        let body = AttribteBodyResponse::Relation {
            relation_type: body.relation_type.clone(),
            target,
        };
        Self { id, body }
    }
}
