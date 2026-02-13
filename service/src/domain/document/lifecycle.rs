use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum PublicationState {
    /// Still being edited
    Draft { revision: i32 },

    /// Published, changes create new revision
    Published {
        revision: i32,
        published_at: DateTime<Utc>,
        published_by: Option<UserId>,
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

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct UserId(pub String);

impl From<String> for UserId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for UserId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<UserId> for String {
    fn from(value: UserId) -> Self {
        value.0
    }
}
