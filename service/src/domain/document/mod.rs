pub mod content;
pub mod error;
pub mod lifecycle;

use std::collections::HashMap;

use chrono::Utc;
use luminair_common::DocumentTypeId;
use serde::{Deserialize, Serialize};

use crate::domain::document::{
    content::ContentValue,
    error::DocumentError,
    lifecycle::{AuditTrail, PublicationState, UserId},
};

/// A DocumentInstance: one actual row of data
/// An instance of a DocumentType
/// Example: One specific Partner (with idno "1234567890123")
#[derive(Debug, Clone)]
pub struct DocumentInstance {
    /// Primary key: unique within this DocumentType
    pub id: DocumentInstanceId,

    /// Which DocumentType does this instance conform to?
    pub document_type_id: DocumentTypeId,

    /// The actual field values: field_name → value
    pub content: DocumentContent,

    /// System/infrastructure metadata about this instance
    pub audit: AuditTrail,
}

/// Wrapper to prevent ID confusion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentInstanceId(pub i64);

impl From<i64> for DocumentInstanceId {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

/// The actual data payload of a document
#[derive(Debug, Clone)]
pub struct DocumentContent {
    /// All fields with their values
    pub fields: HashMap<String, ContentValue>,

    /// Publishing state (if draft_and_publish is enabled)
    pub publication_state: PublicationState,
}

impl DocumentInstance {
    pub fn new(
        id: DocumentInstanceId,
        document_type_id: DocumentTypeId,
        content: DocumentContent,
    ) -> Self {
        Self {
            id,
            document_type_id,
            content,
            audit: AuditTrail {
                created_at: Utc::now(),
                created_by: None,
                updated_at: Utc::now(),
                updated_by: None,
                version: 1,
            },
        }
    }

    /// Domain invariant: validate instance against its type
    /*
    pub fn validate(&self, document_type: &DocumentType) -> Result<(), DocumentError> {
        for (attr_name, attribute) in &document_type.attributes {
            match attribute {
                Attribute::Text { required: true, .. } => {
                    if !self.content.fields.contains_key(attr_name) {
                        return Err(DocumentError::MissingRequiredField(attr_name.clone()));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    */

    /// Publish a draft
    pub fn publish(&mut self, user_id: Option<UserId>) -> Result<(), DocumentError> {
        match &self.content.publication_state {
            PublicationState::Draft { .. } => {
                self.content.publication_state = PublicationState::Published {
                    revision: self.audit.version,
                    published_at: Utc::now(),
                };
                self.audit.updated_by = user_id;
                self.audit.version += 1;
                Ok(())
            }
            PublicationState::Published { .. } => Err(DocumentError::AlreadyPublished),
        }
    }

    /// Unpublish back to draft
    pub fn unpublish(&mut self) -> Result<(), DocumentError> {
        match &self.content.publication_state {
            PublicationState::Published { .. } => {
                self.content.publication_state = PublicationState::Draft {
                    revision: self.audit.version,
                };
                self.audit.version += 1;
                Ok(())
            }
            PublicationState::Draft { .. } => Err(DocumentError::AlreadyDraft),
        }
    }
}
