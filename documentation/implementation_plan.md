# Persistence Refactoring — Draft-Publish Conformance

Align `service/src/infrastructure/persistence` with the documented **main table + snapshots table** pattern described in [`draft-publish.md`](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/documentation/draft-publish.md).

---

## Current State Analysis

### What the documentation mandates

| Concept | Documented behaviour |
|---------|---------------------|
| **Main table status** | `DRAFT`, `PUBLISHED`, or `MODIFIED` |
| **`MODIFIED` state** | Document has published snapshot(s) **plus** pending draft changes |
| **Edit operation** | `status = CASE WHEN revision = 0 THEN 'DRAFT' ELSE 'MODIFIED' END`, clear `published_at` / `published_by` |
| **Publish step 1** | `INSERT INTO {type}_snapshots` with content at publish time, returns `snapshot_id` |
| **Publish step 2** | `UPDATE` main table: `status = 'PUBLISHED'`, bump `version`, set `revision`, `published_at`, `published_by` |
| **Publish relations** | `INSERT INTO {type}_snapshot_{attr}` — copy draft relation set into snapshot relation set |
| **Read published** | Query snapshots table (`{type}_snapshots`), latest revision via `MAX(revision)` sub-query |
| **Read draft/editor** | Query main table only |
| **Relation tables** | Draft: `{type}_{attr}` (owning_document_id → target_document_id). Snapshot: `{type}_snapshot_{attr}` (snapshot_id → target_document_id) |

### What the current code does — and the gaps

#### Gap 1 — `MODIFIED` status is missing ([`repository.rs:423-432`](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/repository.rs#L423-L432))

`main_status_value()` maps only `Draft → "DRAFT"` and `Published → "PUBLISHED"`.
**The `MODIFIED` state is never written.** After an edit to a previously-published document the
`PublicationState` in code is `Draft { revision: N }` (N > 0), yet the DB column should read
`MODIFIED` according to the docs.

> **Root cause:** `PublicationState` only has two variants. The domain model does not distinguish
> "first draft" from "draft-after-publish" (i.e. `MODIFIED`). `revision > 0` in the `Draft`
> variant is the implicit `MODIFIED` signal, but `main_status_value` ignores it.

#### Gap 2 — Edit does not clear `published_at` / `published_by` on main table ([`repository.rs:195-233`](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/repository.rs#L195-L233))

The `update()` method builds `column_values` without ever including `published_at` or
`published_by`. The documentation requires:
```sql
SET published_at = NULL, published_by_id = NULL
```
on every non-publish edit so the main table accurately tracks that there are pending changes.

#### Gap 3 — Insert does not write `revision` / `published_at` / `published_by` ([`write.rs:47-62`](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/builders/write.rs#L47-L62))

`main_insert_columns()` omits `revision`, `published_at`, `published_by_id`.  
The initial row must be `revision = 0, published_at = NULL, published_by_id = NULL`.

#### Gap 4 — `store_snapshot_for_published_instance` is called on every `update` that happens to be `Published` ([`repository.rs:200-204`](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/repository.rs#L200-L204))

The snapshot is inserted before the main table `UPDATE`. This is correct but fragile — if
re-publishing the same revision (e.g. calling `publish()` twice without editing), a second
snapshot row will be attempted against the `UNIQUE (document_id, revision)` constraint.
The uniqueness constraint should guard against duplicates, but the error surface is currently
swallowed as a `DatabaseError`. The method should also atomically wrap the two operations.

#### Gap 5 — Snapshot relations are never frozen on publish ([`repository.rs`](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/repository.rs))

When publishing, the documentation mandates:
```sql
INSERT INTO article_snapshot_categories (snapshot_id, target_document_id)
SELECT $snapshot_id, target_document_id
FROM article_categories WHERE owning_document_id = $document_id;
```
No such operation exists anywhere in the current persistence code.

#### Gap 6 — `fetch_relations` always reads draft relation tables ([`relations.rs:15-41`](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/builders/relations.rs#L15-L41))

`query_find_related_documents` queries `{type}_{attr}` (draft relation table) regardless of
whether the caller is serving a published or draft query. For published documents, the query
should read `{type}_snapshot_{attr}` joined on `snapshot_id`.

#### Gap 7 — `find_by_id` for `Published` uses wrong column alias ([`find.rs:80-98`](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/builders/find.rs#L80-L98))

`snapshot_select_columns` references `(\"m\", ...)` aliases (e.g. `m.id`, `m.document_id`,
`m.created_at`) which do not exist when querying **only** the snapshots table without a JOIN to
the main table. `query_find_document_by_id` for `Published` just does `FROM article_snapshots s`
with no join, so `m.*` columns would fail at runtime. This is a latent bug exposed only when
published single-document lookups actually include `id` from the snapshots table.

---

## User Review Required

> [!IMPORTANT]
> **`MODIFIED` status in domain vs. persistence.** The current `PublicationState` enum has two
> variants: `Draft { revision }` and `Published { ... }`. Adding `MODIFIED` to the persistence
> layer can be done in one of two ways:
> 1. **(Recommended — no domain change)** Keep the enum as-is. Derive the DB status string from
>    `Draft { revision }` — if `revision > 0` write `"MODIFIED"`, else `"DRAFT"`. This is
>    pure persistence logic, invisible to the domain.
> 2. **Add `Modified` variant to `PublicationState`** — makes the domain reflect editorial
>    semantics explicitly but changes the public domain API and the application service's
>    `update()` implementation.
>
> The plan below assumes **Option 1** unless you prefer Option 2.

> [!WARNING]
> **Snapshot relation tables must exist.** Gap 5 (freezing relations) requires
> `{type}_snapshot_{attr}` tables. If these are not yet created by the migration crate,
> that must be addressed there separately before the persistence code can write to them.
> Confirm whether the migration crate already creates snapshot relation tables.

> [!CAUTION]
> **Atomicity of publish.** The two-step publish (insert snapshot, then update main table)
> is currently not wrapped in a transaction. A crash between the two steps would leave the
> DB in an inconsistent state. The refactoring should wrap both steps in a `sqlx` transaction.

---

## Open Questions

1. **Snapshot relation tables in migration** — does `src/migration` already generate
   `{type}_snapshot_{attr}` tables? The plan adds writes to them but cannot verify without
   inspecting the migration crate.
2. **`snapshot_id` RETURNING** — `build_snapshot_insert` currently uses `.execute()` which
   discards the returned `snapshot_id` (needed for freezing relations). Should we change
   `store_snapshot_for_published_instance` to return the `snapshot_id`?
3. **Re-publishing guard** — should the service layer prevent calling `publish()` when
   already `Published`, or should the repository handle the UNIQUE constraint gracefully?
   Currently the TODO comment in `implementation.rs:144` acknowledges this.

---

## Proposed Changes

### `infrastructure/persistence/builders/write.rs`

#### [MODIFY] [write.rs](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/builders/write.rs)

- Add `revision`, `published_at` (`PUBLISHED_FIELD_NAME`), and `published_by_id`
  (`PUBLISHED_BY_FIELD_NAME`) to `main_insert_columns()` so every new row carries the
  correct initial values (`revision = 0`, both timestamps `NULL`).
- Update `insert_document` callers (only `repository.rs::insert`) to supply three new `Expr`
  values: `Expr::from(0i32)`, `Expr::null()`, `Expr::null()`.

---

### `infrastructure/persistence/repository.rs`

#### [MODIFY] [repository.rs](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/repository.rs)

**`main_status_value()`** — fix the three-way status mapping:
```rust
fn main_status_value(&self, _document_type: &DocumentType, instance: &DocumentInstance) -> &'static str {
    match &instance.content.publication_state {
        PublicationState::Published { .. } => "PUBLISHED",
        PublicationState::Draft { revision } if *revision > 0 => "MODIFIED",
        PublicationState::Draft { .. } => "DRAFT",
    }
}
```

**`update()`** — add `published_at` / `published_by` clearing to the column set on non-publish
edits (i.e., when state is `Draft`):
```rust
// When state is Draft, clear publication metadata on main table
if let PublicationState::Draft { revision } = &instance.content.publication_state {
    column_values.push((REVISION_FIELD_NAME.into(), (*revision).into()));
    column_values.push((PUBLISHED_FIELD_NAME.into(), Expr::null()));
    column_values.push((PUBLISHED_BY_FIELD_NAME.into(), Expr::null()));
}
```

**`update()` — when `Published`** — also update `revision`, `published_at`, `published_by` on
the main table:
```rust
if let PublicationState::Published { revision, published_at, published_by } = &instance.content.publication_state {
    column_values.push((REVISION_FIELD_NAME.into(), (*revision).into()));
    column_values.push((PUBLISHED_FIELD_NAME.into(), (*published_at).into()));
    column_values.push((PUBLISHED_BY_FIELD_NAME.into(), /* option → Expr */));
}
```

**`store_snapshot_for_published_instance()`** — change from `.execute()` to `.fetch_one()`
with `RETURNING snapshot_id` so the `snapshot_id` is captured for freezing relations.

**New method `freeze_snapshot_relations()`** — for each owning relation in `document_type.relations`:
```sql
INSERT INTO {type}_snapshot_{attr} (snapshot_id, target_document_id)
SELECT $snapshot_id, target_document_id
FROM {type}_{attr} WHERE owning_document_id = $document_id;
```

**`update()` — wrap publish in a transaction** — use `self.database.database_pool().begin()`
to execute the snapshot insert + main table update + relation freeze atomically.

**`insert()`** — pass the three new columns (`revision = 0`, `published_at = NULL`,
`published_by = NULL`) to `insert_document`.

---

### `infrastructure/persistence/builders/relations.rs`

#### [MODIFY] [relations.rs](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/builders/relations.rs)

**`query_find_related_documents()`** — add a `source: RelationSource` parameter:
```rust
pub enum RelationSource { Draft, Snapshot(i64) /* snapshot_id */ }
```
- `Draft` → keep existing join on `{type}_{attr}` (owning_id → related main table)
- `Snapshot(snapshot_id)` → join `{type}_snapshot_{attr}` on `snapshot_id` →
  `target_document_id` → related main table

Add a new builder function:
```rust
pub fn insert_snapshot_relation(
    document_type: &DocumentType,
    relation_attr: &AttributeId,
    snapshot_id: i64,
    document_uuid: Uuid, // target_document_id
) -> (String, SqlxValues)
```

And a bulk-copy function:
```rust
pub fn copy_draft_relations_to_snapshot(
    document_type: &DocumentType,
    relation_attr: &AttributeId,
    snapshot_id: i64,
    document_id: Uuid, // owning document uuid
) -> (String, SqlxValues)
```
which generates:
```sql
INSERT INTO {type}_snapshot_{attr} (snapshot_id, target_document_id)
SELECT $snapshot_id, target_document_id
FROM {type}_{attr} WHERE owning_document_id = $document_id
```

---

### `infrastructure/persistence/builders/find.rs`

#### [MODIFY] [find.rs](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/builders/find.rs)

**`snapshot_select_columns()`** — fix column aliases: the snapshot-only query has no `m.*`
join. Change to use only `"s"` alias (snapshot table alias) for snapshot-native columns:
- `s.snapshot_id` (new — needed to pass to relation freeze)
- `s.document_id`
- `s.revision`
- `s.published_at`
- `s.published_by_id`
- `s.<field_columns>...`

For `id`, `created_at`, `updated_at`, `version` (which live only on the main table), a
LEFT JOIN to the main table is added — consistent with `query_find_document_by_criteria` for
Published which already does this JOIN.

**`query_find_document_by_id()` for Published** — add the same JOIN on main table as
`query_find_document_by_criteria` already does so `m.id`, `m.created_at`, etc. resolve.

---

### `infrastructure/persistence/mapping/reader.rs`

#### [MODIFY] [reader.rs](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/infrastructure/persistence/mapping/reader.rs)

- `parse_publication_state()` — unchanged (already reads `published_at` / `published_by` /
  `revision` correctly for both draft and published rows).
- Adjust field reads to handle the unified column layout (both query paths now return the
  same columns, just populated from different tables).

---

### `domain/repository.rs` (optional, for clarity)

#### [MODIFY] [repository.rs](file:///c:/Users/demiu/my-rust-projects/luminair-cms/luminair-service/src/service/src/domain/repository.rs)

- Add `SnapshotConflict` variant to `RepositoryError` (for UNIQUE violation on
  `document_id, revision`).
- Update doc comment on `update()` to mention that publish triggers snapshot creation + relation
  freeze and wraps in a transaction.

---

## Verification Plan

### Automated Tests

No existing automated test suite was found in the persistence module. After refactoring:
```powershell
cargo build -p service
```
Confirms the code compiles (compile-time `sqlx` query checks will surface schema mismatches
if `DATABASE_URL` is set).

### Manual Verification

1. **Create** a document — confirm DB row has `status='DRAFT'`, `revision=0`,
   `published_at=NULL`, `published_by_id=NULL`.
2. **Edit** a never-published document — confirm `status` stays `'DRAFT'`, `revision=0`.
3. **Publish** — confirm:
   - A row appears in `{type}_snapshots` with `revision=1`, correct content.
   - Main table: `status='PUBLISHED'`, `revision=1`, `published_at` set.
   - Snapshot relation tables populated (if relations exist).
4. **Edit a published document** — confirm main table: `status='MODIFIED'`, `published_at=NULL`.
5. **Re-publish** — confirm `{type}_snapshots` gets `revision=2`. Main table: `status='PUBLISHED'`,
   `revision=2`. Old snapshot row intact.
6. **Read published via API** — confirm response comes from snapshots table (matches snapshot
   content, not draft edits).
7. **Read draft via editor API** — confirm response from main table includes pending changes.
