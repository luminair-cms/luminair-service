# Draft and Publish Workflow

## Overview

Luminair implements a draft-and-publish workflow for document types that have `draftAndPublish` enabled in their schema options. This allows content creators to work on drafts before making them publicly available, providing a clean separation between work-in-progress and published content.

## Core Concepts

### PublicationState

The `PublicationState` enum represents the current publication status of a document's content:

```rust
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
```

- **Draft**: Content is being edited and not yet published. The `revision` field tracks the draft revision number.
- **Published**: Content is live and publicly accessible. Includes publication timestamp and the user who performed the publish action.

### AuditTrail

Separate from publication state, `AuditTrail` tracks system metadata for audit purposes:

```rust
pub struct AuditTrail {
    pub created_at: DateTime<Utc>,
    pub created_by: Option<UserId>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<UserId>,
    pub version: i32,
}
```

## Workflow States

### Document Lifecycle

1. **Creation**: New documents start in `Draft { revision: 0 }` state
2. **Editing**: Multiple edits can occur while in draft state, each incrementing the `version` in `AuditTrail`
3. **Publishing**: Transition from draft to published state
4. **Post-Publish Editing**: Further edits create new draft revisions while maintaining the published version

### State Transitions

```
Draft (rev: 0) → [Edit] → Draft (rev: 0) → [Publish] → Published (rev: 1)
                                                        ↓
                                               Draft (rev: 2) → [Publish] → Published (rev: 2)
```

## Key Differences: Revision vs Version

| Aspect | `revision` (PublicationState) | `version` (AuditTrail) |
|--------|-------------------------------|-------------------------|
| **Purpose** | Tracks publication cycle iteration | Tracks overall change history |
| **Scope** | Only for published versions | Every state change (edit, publish, unpublish) |
| **When Updated** | When transitioning Draft → Published | Every operation that modifies the document |
| **Example** | Publishing creates revision 1, 2, 3... | Increments with edits AND publishes |

### Concrete Example

```
1. Create document → version: 1, state: Draft { revision: 1 }
2. Edit content  → version: 2, state: Draft { revision: 1 }  (revision stays same)
3. Edit again    → version: 3, state: Draft { revision: 1 }  (revision stays same)
4. Publish       → version: 4, state: Published { revision: 1, published_at: T1 }
5. Edit          → version: 5, state: Draft { revision: 2 }   (revision increments!)
6. Publish       → version: 6, state: Published { revision: 2, published_at: T2 }
```

## Database Schema

For document types with draft-and-publish enabled, the database table includes additional columns:

| Field | Type | Description |
|-------|------|-------------|
| id | serial | Primary key |
| document_id | uuid | Document instance identifier |
| published_at | timestamp | Publication timestamp |
| published_by_id | integer | User who performed publish |
| revision | integer | Publication revision number |
| created_at | timestamp | Creation timestamp |
| updated_at | timestamp | Last update timestamp |
| created_by_id | integer | User who created |
| updated_by_id | integer | User who last updated |
| version | integer | Overall version number |

## Publishing Logic

The `publish()` method on `DocumentInstance` handles the state transition:

```rust
pub fn publish(&mut self, user_id: Option<UserId>) -> Result<(), DocumentError> {
    match &self.content.publication_state {
        PublicationState::Draft { .. } => {
            self.content.publication_state = PublicationState::Published {
                revision: self.audit.version,
                published_at: Utc::now(),
                published_by: user_id,
            };
            self.audit.version += 1;
            Ok(())
        }
        PublicationState::Published { .. } => Err(DocumentError::AlreadyPublished),
    }
}
```

Key behaviors:
- Only draft documents can be published
- Publishing sets the revision to the current audit version
- Publishing increments the overall version counter
- Published documents cannot be published again (would need unpublish first)

## API Considerations

### Content Visibility

- **Draft content**: Only visible to content editors/administrators
- **Published content**: Publicly accessible via API endpoints

### Query Parameters

APIs should support filtering by publication state:
- `?status=draft` - Return only draft versions
- `?status=published` - Return only published versions
- Default behavior depends on user permissions

### Relations and Draft-Publish

When draft-and-publish is enabled for document types, relations between documents also respect publication states. A draft document can reference both draft and published related documents, but published documents typically only reference other published documents.

## Schema Configuration

Document types enable draft-and-publish in their schema:

```json
{
  "options": {
    "draftAndPublish": true
  }
}
```

When `draftAndPublish` is `false` or omitted, documents are always considered published and the publication state tracking is disabled.

## Future Extensions

The current implementation provides the foundation for more advanced publishing workflows:

- **Unpublish**: Transition from published back to draft
- **Scheduled Publishing**: Publish at a future date/time
- **Content Approval**: Multi-step approval workflows
- **Version History**: Access to all historical revisions
- **Content Preview**: Preview unpublished changes