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

- **Draft**: Content is being edited and not yet published. `revision` holds the revision number of the last publication this draft is based on (`0` if the document has never been published).
- **Published**: Content is live and publicly accessible. `revision` is the publication sequence number (1 for first publish, 2 for second, etc.). Includes the publication timestamp and the user who performed the action.

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

`revision` (publication counter) and `version` (save counter) are **independent**.
They have different cadences and answer different questions:

| Counter | Increments on | Question answered |
|---------|---------------|-------------------|
| `AuditTrail.version` | every save — edit, publish, unpublish | *How many times has this document been modified?* |
| `PublicationState.revision` | publish only | *Which publication of this document is this?* |

1. **Creation**: `DocumentContent::new` initialises `Draft { revision: 0 }`. `DocumentInstance::new` sets `AuditTrail.version = 1`. `revision = 0` means the document has never been published.
2. **Editing**: Each save increments `AuditTrail.version`. `revision` is unchanged.
3. **Publishing**: `revision` is incremented from its current value (first publish: 0 → 1, second: 1 → 2, …). `AuditTrail.version` is also incremented as this is a save operation. The two increments are independent.
4. **Post-Publish Editing**: The document returns to `Draft { revision: N }` where `N` is the last published revision. Further edits increment only `AuditTrail.version`. `revision` stays at `N` until the next publish.
5. **Unpublish** *(planned)*: Transitions `Published { revision: N }` → `Draft { revision: N }`, increments `AuditTrail.version`.

### State Transitions

```
Create:  Draft { rev: 0, version: 1 }
Edit:    Draft { rev: 0, version: 2 }
Edit:    Draft { rev: 0, version: 3 }
Publish: Published { rev: 1, version: 4 }   ← rev: 0+1=1  version: 3+1=4 (independent)
Edit:    Draft { rev: 1, version: 5 }       ← rev carries last published value
Edit:    Draft { rev: 1, version: 6 }
Publish: Published { rev: 2, version: 7 }   ← rev: 1+1=2  version: 6+1=7 (independent)
```

## Key Differences: Revision vs Version

| Aspect | `revision` (PublicationState) | `version` (AuditTrail) |
|--------|-------------------------------|-------------------------|
| **Purpose** | Publication sequence number — *which publish is this?* | Save sequence number — *how many times was this modified?* |
| **Starting value** | 0 (never published) | 1 (first save on creation) |
| **Increments on** | publish only | every save: edit, publish, unpublish |
| **Relationship** | Independent of `version` | Independent of `revision` |
| **In Draft state** | Holds the last published revision (0 if never published) | Always current |
| **In Published state** | Monotonically increasing publication number (1, 2, 3…) | Always current |

### Concrete Example

```
Op              PublicationState                  AuditTrail.version
──────────────  ────────────────────────────────  ──────────────────
Create          Draft { revision: 0 }             1
Edit            Draft { revision: 0 }             2
Edit            Draft { revision: 0 }             3
Publish         Published { revision: 1 }         4   ← rev 0→1, ver 3→4 independently
Edit            Draft { revision: 1 }             5   ← rev carries last published value
Publish         Published { revision: 2 }         6   ← rev 1→2, ver 5→6 independently
Edit            Draft { revision: 2 }             7
Edit            Draft { revision: 2 }             8
Publish         Published { revision: 3 }         9   ← rev 2→3, ver 8→9 independently
```

## Publishing Logic

The `publish()` method on `DocumentInstance` handles the state transition:

```rust
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
```

Key behaviors:
- Only draft documents can be published.
- `revision` and `version` are incremented independently. Publishing increments both, but for completely different reasons and from different base values.
- `Published.revision` is incremented from `Draft.revision` (the last published revision), not from `version`.
- `AuditTrail.version` is incremented because publish is a save operation, not because of any relationship to `revision`.
- Edits increment only `AuditTrail.version`. `revision` in the `Draft` state is frozen at the last published value until the next publish.
- Published documents cannot be published again (call `unpublish()` first).

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