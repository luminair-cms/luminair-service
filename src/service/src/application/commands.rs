use std::collections::HashMap;
use luminair_common::{AttributeId, DocumentType};
use crate::domain::document::content::ContentValue;
use crate::domain::document::DocumentInstanceId;
use crate::domain::document::lifecycle::UserId;
use crate::domain::query::DocumentInstanceQuery;

pub struct FindDocumentsCommand {
    pub document_type: &'static DocumentType,
    pub populate: Option<Vec<AttributeId>>,
    pub populate_filters: Option<HashMap<AttributeId, crate::domain::query::FilterExpression>>,
    pub query: DocumentInstanceQuery,
}

pub struct FindByIdCommand {
    pub document_type: &'static DocumentType,
    pub document_instance_id: DocumentInstanceId,
    pub populate: Option<Vec<AttributeId>>,
    pub populate_filters: Option<HashMap<AttributeId, crate::domain::query::FilterExpression>>,
    pub query: DocumentInstanceQuery,
}

pub struct CreateDocumentCommand {
    pub document_type: &'static DocumentType,
    pub fields: HashMap<AttributeId, ContentValue>,
    pub user_id: Option<UserId>,
}

pub struct UpdateDocumentCommand {
    pub document_type: &'static DocumentType,
    pub document_id: DocumentInstanceId,
    pub fields: HashMap<AttributeId, ContentValue>,
    pub user_id: Option<UserId>,
}

pub struct DeleteDocumentCommand {
    pub document_type: &'static DocumentType,
    pub document_instance_id: DocumentInstanceId,
}

pub struct PublishDocumentCommand {
    pub document_type: &'static DocumentType,
    pub document_id: DocumentInstanceId,
    pub user_id: Option<UserId>,
}

pub struct ModifyRelationsCommand {
    pub document_type: &'static DocumentType,
    pub document_id: DocumentInstanceId,
    pub operations: HashMap<AttributeId, RelationOperation>,
}

#[derive(Debug)]
pub enum RelationOperation {
    /// Partial update: add and/or remove specific relations.
    ConnectDisconnect {
        connect: Vec<DocumentInstanceId>,
        disconnect: Vec<DocumentInstanceId>,
    },
    /// Full replacement: remove all existing relations and replace with this set.
    Set(Vec<DocumentInstanceId>),
}

pub struct CreateDocumentWithRelationsCommand {
    pub document_type: &'static DocumentType,
    pub fields: HashMap<AttributeId, ContentValue>,
    pub relation_operations: HashMap<AttributeId, RelationOperation>,
    pub user_id: Option<UserId>,
}

pub struct UpdateDocumentWithRelationsCommand {
    pub document_type: &'static DocumentType,
    pub document_id: DocumentInstanceId,
    pub fields: HashMap<AttributeId, ContentValue>,
    pub relation_operations: HashMap<AttributeId, RelationOperation>,
    pub user_id: Option<UserId>,
}