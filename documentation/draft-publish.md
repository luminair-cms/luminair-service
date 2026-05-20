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
- **Draft**: Content is being edited and not yet published. New documents start in `Draft { revision: 0 }`.
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

1. **Creation (in-memory)**: When creating a `DocumentContent` in memory, the content defaults to `Draft { revision: 0 }` (see `DocumentContent::new`). When a `DocumentInstance` is constructed via `DocumentInstance::new` the `AuditTrail.version` is initialized to `1` (see the runtime constructor).
2. **Persistence semantics**: After persisting a newly created instance, the stored database row uses `revision = 1` and `version = 1`. When loading rows from the database the loader (`parse_publication_state`) returns `PublicationState::Draft { revision: 1 }` for drafts.
3. **Editing**: Each edit operation in the service should increment the `AuditTrail.version` counter.
4. **Publishing**: Publishing increments `AuditTrail.version` and sets `PublicationState::Published.revision` to the `AuditTrail.version` value used for the publish operation (the code sets `Published.revision = audit.version` and then increments `audit.version`).
5. **Post-Publish Editing**: Further edits after publishing increment `AuditTrail.version` and return the content to `Draft` while the previously published revision remains available.

### State Transitions

```
Draft (in-memory: rev: 0, audit.version: 1) → [Persist] → Draft (persisted: rev: 1, version: 1) → [Edit] → Draft (rev: 1, version: 2) → [Publish] → Published (rev: 2, version: 2)
                                                                                                     ↓
                                                                                        [Edit] → Draft (rev: 2, version: 3) → [Publish] → Published (rev: 3, version: 3)
```

## Key Differences: Revision vs Version

| Aspect | `revision` (PublicationState) | `version` (AuditTrail) |
|--------|-------------------------------|-------------------------|
| **Purpose** | Identifies the published revision number | Monotonic counter tracking edits and publish operations |
| **Scope** | Only set when a publish occurs | Incremented on every edit and publish operation |
| **When Updated** | On transitioning Draft → Published (set to the new `AuditTrail.version`) | On every operation that modifies the document (edits and publishes) |
| **Example** | Publishing creates new revision numbers (e.g., 3, 5...) | Increments with each edit and each publish |

### Concrete Example (reflecting code paths)

```
1. Create in memory: content.publication_state = Draft { revision: 0 }, audit.version = 1
2. Persist: stored row set to revision = 1, version = 1
3. Edit (service applies change): audit.version -> 2, content remains Draft { revision: 1 }
4. Publish: publish sets Published.revision = audit.version (2), then increments audit.version -> 3
5. Edit: audit.version -> 4, content Draft { revision: 2 }
6. Publish: Published.revision = 4, audit.version -> 5
```

## Publishing Logic

The `publish()` method on `DocumentInstance` handles the state transition:

```rust
pub fn publish(&mut self, user_id: Option<UserId>) -> Result<(), DocumentError> {
    match &self.content.publication_state {
        PublicationState::Draft { .. } => {
            // Increment audit.version for the publish operation
            self.audit.version += 1;
            self.content.publication_state = PublicationState::Published {
                revision: self.audit.version,
                published_at: Utc::now(),
                published_by: user_id,
            };
            Ok(())
        }
        PublicationState::Published { .. } => Err(DocumentError::AlreadyPublished),
    }
}
```

Key behaviors:
- Only draft documents can be published
- Publishing increments the overall `AuditTrail.version` and sets `PublicationState.revision` to the new `version`
- Edits increment `AuditTrail.version` without changing the currently published `revision` until the next publish
- Published documents cannot be published again (would need unpublish first)

## API Considerations

### Content Visibility

- **Draft content**: Only visible to content editors/administrators
- **Published content**: Publicly accessible via API endpoints

### Query Parameters

APIs should support filtering by publication state:
- `?status=published` — Return published versions only (default). Used by public api
- `?status=draft`:
   - For filter many documents. Returns a draft version if it exists.  Use case: give me documents what need to approve
   - For find by ID: Returns a draft version if it exists. Also returns a published version. Use case: UI for editing a single document

### Relations and Draft-Publish

When draft-and-publish is enabled for document types, relations between documents also respect publication states.
A draft document can work with relation values independently from the currently published version.

For connected collections (`hasMany` / `hasOne` owning relations):

- relation additions and removals are applied against the draft version of the owning document
- the draft relation set may include references to both draft and published related documents
- when the owning document is published, the relation set is synchronized to the published row
- published documents should expose a stable relation graph representing the last published state

This means connected collections are versioned along with the document instance:
- the same `document_id` may have multiple main table rows (draft + published)
- relation rows point at the concrete main row IDs (`owning_id` / `inverse_id`)
- editing relations in draft does not immediately change published visibility

If the related document type also has draft-and-publish enabled, then the service should resolve relation visibility according to each document's own publication state. In practice, published documents and published related items are what end users see, while editors can preview draft relation changes before publishing.

### Clarification rules for connections and Draft-Publish

1. **Relations are versioned per document, not per relation.**

A relation addition/removal is part of the owning document's draft. Publishing the owning document is what "approves" the relation change. The related document's state is irrelevant to this.

2. **At query time, resolve visibility by the requester's context.**

status=published → only return relations where the related document also has a published row
status=draft → return all relations including those pointing at draft-only targets

This is already implied in your draft-publish.md but worth making it an explicit rule rather than a footnote.

3. **Connecting two published documents does NOT change their publication state.**

The connection is recorded on the owning document's draft row. If that document has no pending draft yet, creating the relation implicitly creates a new draft revision. Publishing that draft then makes the connection visible in the published graph. This is the key insight — you never mutate the published row directly.

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