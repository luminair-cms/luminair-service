use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PublicationState {
    /// Still being edited
    Draft { revision: i32 },

    /// Published, changes create new revision
    Published {
        revision: i32,
        published_at: DateTime<Utc>,
    },
}

/// System metadata: WHO did WHAT WHEN
/// This is infrastructure/audit concern, not domain logic
#[derive(Debug, Clone)]
pub struct AuditTrail {
    pub created_at: DateTime<Utc>,
    pub created_by: Option<UserId>,

    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<UserId>,

    pub version: i32,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct UserId(pub i32);
