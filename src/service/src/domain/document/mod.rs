pub mod content;
pub mod error;
pub mod lifecycle;

use std::collections::HashMap;

use crate::domain::document::content::DocumentContent;
use crate::domain::document::{
    error::DocumentError,
    lifecycle::{AuditTrail, PublicationState, UserId},
};
use chrono::Utc;
use luminair_common::AttributeId;
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;

/// A DocumentInstance: one actual row of data
/// An instance of a DocumentType
/// Example: One specific Partner (with idno "1234567890123")
#[derive(Debug, Clone)]
pub struct DocumentInstance {
    /// Primary key: unique within this DocumentType
    pub id: DatabaseRowId,

    /// Unique identifier of this instance, while id is a id of database row
    pub document_id: DocumentInstanceId,

    /// The actual field values: field_name → value
    pub content: DocumentContent,

    /// Document relations
    pub relations: HashMap<AttributeId, Vec<DocumentRelation>>,

    /// System/infrastructure metadata about this instance
    pub audit: AuditTrail,
}

impl DocumentInstance {
    pub(crate) fn with_relations(
        self,
        relations: HashMap<AttributeId, Vec<DocumentInstance>>,
    ) -> DocumentInstance {
        let relations = relations
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().map(|i| i.into()).collect()))
            .collect();
        Self { relations, ..self }
    }
}

/// Wrapper to prevent ID confusion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DatabaseRowId(pub i64);

impl From<i64> for DatabaseRowId {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

/// Wrapper to prevent ID confusion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentInstanceId(pub Uuid);

impl From<Uuid> for DocumentInstanceId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl TryFrom<&str> for DocumentInstanceId {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let uuid = Uuid::parse_str(value)?;
        Ok(Self(uuid))
    }
}

impl TryFrom<&String> for DocumentInstanceId {
    type Error = anyhow::Error;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        let uuid = Uuid::parse_str(value)?;
        Ok(Self(uuid))
    }
}

impl From<DocumentInstanceId> for String {
    fn from(value: DocumentInstanceId) -> Self {
        value.0.to_string()
    }
}

impl DocumentInstanceId {
    /// Generate a new time-ordered UUID v7 identifier.
    pub fn generate() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

impl DocumentInstance {
    pub fn new(
        id: DatabaseRowId,
        document_id: DocumentInstanceId,
        content: DocumentContent,
        relations: HashMap<AttributeId, Vec<DocumentRelation>>,
    ) -> Self {
        Self {
            id,
            document_id,
            content,
            relations,
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

    /// Transitions the document from `Draft` to `Published`.
    ///
    /// ## Version and revision — independent counters
    ///
    /// `revision` and `version` are completely independent counters with different
    /// purposes and different cadences:
    ///
    /// - `AuditTrail.version` increments on **every save** (every edit, every publish,
    ///   every unpublish). It answers: *"how many times was this document modified?"*
    /// - `PublicationState::Published.revision` increments **only on publish**. It
    ///   answers: *"which publication of this document is this?"*
    ///
    /// The `revision` field in `Draft { revision }` carries the revision number of the
    /// **last publication this draft is based on** (0 if the document has never been
    /// published). On publish, `revision` is incremented from this value.
    ///
    /// Example — starting from `audit.version = 3`, `Draft { revision: 1 }`
    /// (second edit cycle after first publication):
    /// ```text
    /// publish() → Published { revision: 2 }, audit.version: 4
    /// ```
    ///
    /// ## Errors
    ///
    /// Returns [`DocumentError::AlreadyPublished`] if the document is already in
    /// the `Published` state. Call `unpublish()` before re-publishing.
    pub fn publish(&mut self, user_id: Option<UserId>) -> Result<(), DocumentError> {
        // Extract the current revision from the Draft state.
        // The borrow ends here so we can mutate self below.
        let current_revision = match &self.content.publication_state {
            PublicationState::Draft { revision } => *revision,
            PublicationState::Published { .. } => return Err(DocumentError::AlreadyPublished),
        };

        // Increment version (every save increments version).
        self.audit.version += 1;

        // Revision counter is independent: increment from the draft's last-known
        // published revision, not from version.
        self.content.publication_state = PublicationState::Published {
            revision: current_revision + 1,
            published_at: Utc::now(),
            published_by: user_id,
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum DocumentRelation {
    Id(DocumentInstanceId),
    Instance(DocumentInstance),
}

impl From<DocumentInstance> for DocumentRelation {
    fn from(relation: DocumentInstance) -> Self {
        Self::Instance(relation)
    }
}
