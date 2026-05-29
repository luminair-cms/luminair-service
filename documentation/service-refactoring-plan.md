# Service Crate — Refactoring Plan

## Context

This document describes a planned refactoring of the `service` crate.
It was produced by analysing the crate against the architecture documentation and DDD / hexagonal-architecture principles.

For background, see:
- [Architecture](architecture.md)
- [Domain Model](domain-model.md)
- [Draft and Publish Workflow](draft-publish.md)

---

## Root Cause

Three structural problems share one origin: **the application service layer has no substance**, so logic bleeds upward into HTTP handlers and downward into the repository.

```
HTTP Handler        — should be: thin adapter
    ↓ (business logic leaked up)
ApplicationService  — should be: use-case owner; validates, coordinates, calls domain
    ↓ (business logic leaked down)
Repository          — should be: pure persistence, save/load only
```

The fix is to give each layer a clear, enforced contract and make the service layer earn its existence.

---

## Target Module Structure

```
service/src/
├── main.rs
│
├── domain/                         # Pure domain — no I/O, no framework deps
│   ├── mod.rs
│   ├── document/
│   │   ├── mod.rs                  # DocumentInstance aggregate root + publish()
│   │   ├── content.rs              # DocumentContent, ContentValue, DomainValue
│   │   │                           # + ContentValue::from_json() / to_json()
│   │   ├── lifecycle.rs            # PublicationState, AuditTrail, UserId (nutype)
│   │   └── error.rs                # DocumentError (thiserror)
│   ├── repository.rs               # DocumentsRepository port (trait only)
│   └── query.rs                    # DocumentInstanceQuery, FilterExpression, Sort
│
├── application/                    # Use cases — owns orchestration and business rules
│   ├── mod.rs                      # AppState trait
│   ├── service.rs                  # DocumentsService trait
│   ├── commands.rs                 # Typed command structs for each use case
│   ├── error.rs                    # ServiceError (thiserror)
│   └── implementation.rs           # DocumentsServiceImpl
│
└── infrastructure/                 # I/O adapters — knows framework details
    ├── mod.rs                      # AppStateImpl
    ├── settings.rs
    ├── http/
    │   ├── mod.rs                  # HttpServer
    │   ├── error.rs                # ApiError, ApiSuccess
    │   ├── routes.rs
    │   ├── querystring.rs
    │   └── handlers/
    │       ├── schema/             # renamed from documents/ — serves /api/meta/
    │       │   ├── mod.rs
    │       │   └── dto.rs
    │       └── content/            # renamed from data/ — serves /api/documents/
    │           ├── mod.rs          # thin handlers only (~20-30 lines each)
    │           ├── params.rs       # QueryParams, PaginationParams + parse helpers
    │           ├── request.rs      # HTTP JSON → Command types
    │           └── response.rs     # DocumentInstance → HTTP JSON
    └── persistence/
        ├── mod.rs
        ├── repository.rs           # PostgresDocumentsRepository (thin impl)
        ├── queries/                # SQL builders — organised by concern
        │   ├── mod.rs
        │   ├── find.rs             # SELECT queries
        │   ├── write.rs            # INSERT / UPDATE / DELETE
        │   └── relations.rs        # relation table queries + UUID→rowID resolution
        └── mapping/                # DB ↔ Domain — single place for all field conversions
            ├── mod.rs
            ├── reader.rs           # PgRow → DocumentInstance  (from result.rs)
            └── writer.rs           # ContentValue/DomainValue → Expr (from params.rs)
```

---

## Phase 1 — Fix the Domain Layer

**Goal:** Make the domain compile-safe and self-consistent before touching anything else.

### 1.1 — Fix `DocumentError` with `thiserror`

`DocumentError` currently has no `Display`, no `Error` impl, and is never used.
Replace with a `thiserror`-derived enum that becomes the foundation for the error chain.

```rust
// domain/document/error.rs
#[derive(thiserror::Error, Debug)]
pub enum DocumentError {
    #[error("Missing required field: '{0}'")]
    MissingRequiredField(String),

    #[error("Invalid value for field '{field}': {reason}")]
    InvalidFieldValue { field: String, reason: String },

    #[error("Constraint violated for field '{field}': {reason}")]
    ConstraintViolation { field: String, reason: String },

    #[error("Document is already published")]
    AlreadyPublished,

    #[error("Document is not published")]
    AlreadyDraft,
}
```

### 1.2 — Establish independent `revision` and `version` counters

`revision` (publication counter) and `AuditTrail.version` (save counter) are independent
concepts that must not be derived from each other:

| Counter | Increments on | Answers |
|---------|---------------|---------|
| `AuditTrail.version` | every save — edit, publish, unpublish | *how many times was this document modified?* |
| `PublicationState.revision` | publish only | *which publication of this document is this?* |

`revision` in `Draft { revision }` holds the **last published revision this draft is based
on** (0 if never published). On publish, `revision` is incremented from that value
independently of `version`.

Changes required:
- `DocumentInstance::publish()` — extract `current_revision` from `Draft` state, set
  `Published.revision = current_revision + 1`, increment `audit.version` separately.
- `lifecycle.rs` — add doc comments clarifying the independent-counter semantics.
- `draft-publish.md` — rewrite lifecycle, state transitions, key-differences table,
  concrete example, and `publish()` code sample to reflect the correct model.
- `domain-model.md` — update the "Publication workflow" section.

### 1.3 — Apply `nutype` to `UserId`

The project convention is *nutype for value objects that need validation or sanitisation*,
not *nutype for every newtype*. Whether a type benefits from nutype depends on whether the
inner type has invalid states that need to be rejected:

| Type | Inner type | Invalid states? | `nutype`? |
|------|-----------|-----------------|----------|
| `UserId` | `String` | empty string, leading/trailing whitespace | ✅ |
| `DocumentInstanceId` | `Uuid` | none — `Uuid` guarantees validity at construction | ❌ |
| `DatabaseRowId` | `i64` | none — any `i64` is a valid Postgres row key | ❌ |

`DatabaseRowId` and `DocumentInstanceId` are hand-rolled newtypes that already provide
the required type-safety through the type system. After Phase 1.5, `DatabaseRowId` will
not be visible above the persistence layer at all, making it even less of a concern.

**`UserId`** — apply `nutype` with `sanitize(trim)` and `validate(not_empty)`:

```rust
#[nutype(
    sanitize(trim),
    validate(not_empty),
    derive(Debug, Clone, Hash, Eq, PartialEq, AsRef, Into, Display, Serialize, Deserialize)
)]
pub struct UserId(String);
```

**Call-site changes required:**

- `persistence/result.rs` — replace the tuple-struct constructor `created_by.map(UserId)`
  with `created_by.and_then(|s| UserId::try_new(s).ok())`. DB values that are somehow
  empty are treated as `None` rather than panicking.
- `http/handlers/data/response.rs` — replace `audit.created_by.map(String::from)` with
  `audit.created_by.map(|u| u.into())`. `derive(Into)` provides `Into<String>`, which
  is used here instead of the old manual `From<UserId> for String` impl.

### 1.4 — Centralise the JSON codec on `ContentValue`

`content.rs` now owns both JSON directions as the single canonical codec:

```rust
// domain/document/content.rs
impl ContentValue {
    /// Parse from a JSON value — validates type and applies FieldConstraints.
    pub fn from_json(value: &serde_json::Value, field: &DocumentField)
        -> Result<Self, DocumentError>;
}

impl From<&ContentValue> for serde_json::Value { ... }
impl From<&DomainValue>  for serde_json::Value { ... }
```

Both conversions use an **exhaustive** `match field.field_type { ... }`.
Adding a new `FieldType` variant to `common` produces a compile error here,
preventing silent gaps.

**Changes made:**

- `domain/document/content.rs` — `from_json`, `decode_type`, `check_constraint`,
  `From<&ContentValue>`, `From<&DomainValue>` all live here.
- `infrastructure/http/handlers/data/request.rs` — `build_fields_from_payload` is now
  a thin loop that delegates every field to `ContentValue::from_json`. The 100-line
  `convert_to_content_value` function is deleted. Error type changed from `anyhow::Result`
  to `Result<_, DocumentError>`.
- `infrastructure/http/handlers/data/response.rs` — `From<DomainValue> for JsonValue`
  deleted. The field-mapping loop shrinks from ~20 lines to one.
- `infrastructure/persistence/result.rs` — **bug fix**: `FieldType::Uid` now decodes
  as `DomainValue::Text` (text slug), not `DomainValue::Uuid`. Both codec paths are
  now consistent.

**`FieldConstraint` validation** (pattern, min/max length, min/max integer) is now
applied at JSON parse time inside `check_constraint`, which is called from `from_json`
after the type conversion. Previously this was a commented-out stub.

### 1.5 — Simplify the repository port

**Changes made:**

- `domain/repository/query.rs` → `domain/query.rs` (moved up alongside the repo trait)
- `domain/repository/mod.rs` → `domain/repository.rs` (flattened to single file)
- `domain/mod.rs` gains `pub mod query;`
- All import paths `crate::domain::repository::query::*` updated to `crate::domain::query::*`

**Repository trait changes:**

| Removed | Replaced by |
|---------|-------------|
| `fetch_relations_for_one` | (wrapper, deleted) |
| `fetch_relations_for_many` | `fetch_relations(&[DatabaseRowId], &[AttributeId])` |
| `create(content, user_id)` | `insert(&DocumentInstance)` |
| `update(id, content_updates, user_id)` | `update(&DocumentInstance)` (`todo!()`) |
| `publish(id, user_id)` | (service owns state machine, deleted) |
| `connect(DatabaseRowId, DatabaseRowId)` | `apply_relation_ops(DocumentInstanceId, &ops)` |
| `disconnect(DatabaseRowId, DatabaseRowId)` | same |
| — | `count(&query) -> u64` (new, for pagination) |

`RepositoryError` gains `#[derive(thiserror::Error)]` with `#[error]` attributes.

`RelationOps` and `RelationMap` type alias added to `domain/repository.rs`.

**`apply_relation_ops` is fully implemented** in `PostgresDocumentsRepository`:
- A single `SELECT id WHERE document_id = $1` resolves the owning UUID → row ID
- A batch `SELECT id WHERE document_id = ANY($uuids)` resolves related UUIDs in one query
- `insert_relation_entry` / `delete_relation_entry` reused for the actual DML
- The N+1 `find_by_id` loop in the handler is gone

**`DocumentsService` trait** replaces `connect`/`disconnect` with `modify_relations(doc_type,
document_id: DocumentInstanceId, ops: HashMap<AttributeId, RelationOps>)`. Relation ownership
validation runs in the service before calling `repository.apply_relation_ops`.

**`modify_relations` HTTP handler** rewritten to build a `HashMap<AttributeId, RelationOps>`
from the payload and call the single service method. No `find_by_id` calls, no row ID
management.

**`DocumentInstanceId::generate()`** added to `domain/document/mod.rs` for clean UUID
generation in the service layer (`uuid::Uuid::now_v7()`).

**`builders.rs`** also received the Phase 3.2 bug fix for `query_find_document_by_criteria`
(draft/published filter now correctly uses `PUBLISHED_FIELD_NAME` instead of
`DOCUMENT_ID_FIELD_NAME`) along with three new builder functions:
`query_count_documents`, `query_row_id_by_document_uuid`, `query_row_ids_by_document_uuids`.

---

## Phase 2 — Build the Application Layer

**Goal:** Extract a real `application/` module where all use-case orchestration lives.
HTTP handlers call exactly one service method.

### 2.1 — Move `AppState` out of `domain/`

`AppState` is a composition-root concern, not a domain concept.

**Changes made:**

- `domain/mod.rs` — `AppState` definition removed; the file now contains only the four
  `pub mod` declarations (`document`, `query`, `repository`, `application`).
- `domain/application/mod.rs` — `AppState` trait added above `DocumentsService`,
  with a doc comment explaining why it lives here and a `DocumentTypesRegistry` import.
- Import path `crate::domain::AppState` updated to `crate::domain::application::AppState`
  in all four consumers:
  `infrastructure/mod.rs`, `http/handlers/data/mod.rs`,
  `http/handlers/documents/mod.rs`, `http/routes.rs`.
- `infrastructure/http/mod.rs` continues to import `AppState` via
  `crate::infrastructure::AppState` (a private re-use from the parent module) —
  no change needed there.

### 2.2 — Define `ServiceError`

The application layer must not leak `RepositoryError` upward.

```rust
// application/error.rs
#[derive(thiserror::Error, Debug)]
pub enum ServiceError {
    #[error("Document type not found")]
    DocumentTypeNotFound,

    #[error("Document not found")]
    DocumentNotFound,

    #[error("Relation '{0}' not found")]
    RelationNotFound(String),

    #[error("Relation '{0}' is not an owning relation")]
    NotOwningRelation(String),

    #[error("Validation error: {0}")]
    Validation(#[from] DocumentError),

    #[error("Unique constraint violated: {0}")]
    Conflict(String),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<RepositoryError> for ServiceError { ... }
```

**Changes made:**

- `domain/application/error.rs` - Added `ServiceError` enum and `From<RepositoryError>` impl.

### 2.3 — Define command structs in `application/commands.rs`

Commands are typed inputs for each use case.
They replace raw parameter lists and `serde_json::Value` arguments at the service boundary.

```rust
// application/commands.rs

pub struct FindDocumentsCommand {
    pub document_type: &'static DocumentType,
    pub populate: Option<Vec<AttributeId>>,
    pub query: DocumentInstanceQuery,
}

pub struct FindByIdCommand {
    pub document_type: &'static DocumentType,
    pub id: DocumentInstanceId,
    pub populate: Option<Vec<AttributeId>>,
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
    pub document_id: DocumentInstanceId,
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

pub enum RelationOperation {
    /// Partial update: add and/or remove specific relations.
    ConnectDisconnect {
        connect: Vec<DocumentInstanceId>,
        disconnect: Vec<DocumentInstanceId>,
    },
    /// Full replacement: remove all existing relations and replace with this set.
    Set(Vec<DocumentInstanceId>),
}
```

**Changes made:**

- `domain/application/commands.rs` - Added commands structs and `RelationOperation` enum.


### 2.4 — Redefine `DocumentsService` trait in `application/service.rs`

```rust
// application/service.rs
pub trait DocumentsService: Send + Sync + 'static {
    /// Returns (documents, total_count). total_count is used for pagination metadata.
    fn find(&self, cmd: FindDocumentsCommand)
        -> impl Future<Output = Result<(Vec<DocumentInstance>, u64), ServiceError>> + Send;

    fn find_by_id(&self, cmd: FindByIdCommand)
        -> impl Future<Output = Result<Option<DocumentInstance>, ServiceError>> + Send;

    fn create(&self, cmd: CreateDocumentCommand)
        -> impl Future<Output = Result<DocumentInstanceId, ServiceError>> + Send;

    fn update(&self, cmd: UpdateDocumentCommand)
        -> impl Future<Output = Result<DocumentInstance, ServiceError>> + Send;

    fn delete(&self, cmd: DeleteDocumentCommand)
        -> impl Future<Output = Result<(), ServiceError>> + Send;

    fn publish(&self, cmd: PublishDocumentCommand)
        -> impl Future<Output = Result<DocumentInstance, ServiceError>> + Send;

    fn modify_relations(&self, cmd: ModifyRelationsCommand)
        -> impl Future<Output = Result<(), ServiceError>> + Send;
}
```

**Changes made:**

- `domain/application/service.rs` - Added definition of `DocumentsService` trait.
- Moved `application` module from `domain` module as seperate top-level module.


### 2.5 — Implement `DocumentsServiceImpl` with real business logic

The implementation owns every use case. Responsibilities per method:

**`find`**: runs `repository.find()` and `repository.count()` concurrently via `tokio::join!`,
then enriches results with relations. Returns `(documents, total)`.

**`create`**: validates all required fields are present (fields missing from the payload, not
just those explicitly null), builds a `DocumentInstance`, calls `repository.insert()`.
Field type and constraint validation is already done by `ContentValue::from_json` (Phase 1.4).

**`update`**: loads the existing instance, applies field updates, increments `AuditTrail.version`,
calls `repository.update()`.

**`publish`**: loads the instance via `repository.find_by_id()`, calls
`instance.publish(user_id)?` (the domain method — already correctly implemented), calls
`repository.update()` to persist the mutated aggregate. **The repository has no concept of
"publish".**

**`modify_relations`**: validates each field is an owning relation (moved from handler),
builds the `HashMap<AttributeId, RelationOps>`, calls `repository.apply_relation_ops()` once.
The N+1 loop disappears.

**Implemented**

---

## Phase 3 — Fix the Persistence Layer

**Goal:** Fix three correctness bugs and reorganise into `queries/` + `mapping/` sub-modules.

### 3.1 — Fix Bug: Wrong JOIN condition in `query_find_related_documents`

The JOIN condition `r.owning_id = r.inverse_id` is a self-join on the relation table.

```rust
// WRONG — joins the relation table to itself:
ColumnRef::from(("r", OWNING_ID_FIELD_NAME))
    .equals(ColumnRef::from(("r", INVERSE_ID_FIELD_NAME)))

// CORRECT — joins the relation table to the related document table:
ColumnRef::from(("r", INVERSE_ID_FIELD_NAME))
    .equals(ColumnRef::from(("m", ID_FIELD_NAME)))
```

**Fixed**

### 3.2 — Fix Bug: Wrong column for draft/published filter in `query_find_document_by_criteria`

The filter uses `DOCUMENT_ID_FIELD_NAME IS NULL` to detect drafts, but `document_id` is a
UUIDv7 primary key and is never `NULL`. Replace with `PUBLISHED_FIELD_NAME` (`published_at`)
to match the already-correct implementation in `query_find_document_by_id`.

**Already fixed**

### 3.3 — Fix Bug: Draft `revision` hardcoded to `1` in `parse_publication_state`

The `revision` column is read from the database but then discarded for draft rows:

```rust
// WRONG:
None => PublicationState::Draft { revision: 1 },

// CORRECT — use the value already fetched from the DB:
None => PublicationState::Draft { revision },
```

**Fixed**

### 3.4 — Reorganise into `builders/` and `mapping/`

The persistence module is reorganised so that SQL-building lives separately from
DB ↔ Domain translation. The directory originally proposed as `queries/` is
named `builders/` to match the existing terminology in the codebase.

| Old location | New location |
|---|---|
| `builders.rs` SELECT functions | `builders/find.rs` |
| `builders.rs` INSERT/DELETE functions | `builders/write.rs` |
| `builders.rs` relation functions (find / insert / delete / row-id resolution) | `builders/relations.rs` |
| `result.rs` | `mapping/reader.rs` |
| `params.rs` | `mapping/writer.rs` |

**Fixed**

`builders/relations.rs` now owns every relation-table SQL builder:
`query_find_related_documents`, `insert_relation_entry`, `delete_relation_entry`,
`query_row_id_by_document_uuid`, and `query_row_ids_by_document_uuids`.
The row-ID resolution queries were moved out of `find.rs` because they only
exist to support `apply_relation_ops`.

`PostgresDocumentsRepository::apply_relation_ops` resolves all
`DocumentInstanceId` values to `DatabaseRowId` via one single-row lookup for the
owning document and one batch `SELECT id FROM table WHERE document_id = ANY($1)`
for the related documents, then applies connect / disconnect operations via
`insert_relation_entry` / `delete_relation_entry`. (Wrapping the DML in a single
transaction is deferred — there is no per-call transaction yet.)

### 3.5 — Implement `insert`, `update`, `count`

- `insert` accepts a full `DocumentInstance` (all fields pre-set by the service).
- `update` builds `UPDATE SET ... WHERE document_id = $id` via sea-query.
- `count` builds `SELECT COUNT(*) FROM table` sharing the same `WHERE` predicate as `find`.

**Fixed**

`builders/write.rs` gains `update_document(document, document_id, column_values)` —
takes a `Vec<(DynIden, Expr)>` of writable columns and emits
`UPDATE {table} SET ... WHERE document_id = $id`.

`PostgresDocumentsRepository::update` mirrors `insert`: builds the column/value
list from `updated_at`, `version`, the publication state (when applicable), and
all dynamic fields, then executes via `update_document`. Returns
`RepositoryError::DocumentInstanceNotFound` when zero rows are affected.

---

## Phase 4 — Slim the HTTP Layer

**Goal:** Each handler becomes a pure adapter: parse → call one service method → serialize.

### 4.1 — Rename handler modules

| Old name | New name | Reason |
|---|---|---|
| `handlers/data/` | `handlers/content/` | serves `/api/documents/` — content entries |
| `handlers/documents/` | `handlers/schema/` | serves `/api/meta/documents/` — type metadata |

**Implemented**

### 4.2 — Extract shared parsing helpers into `params.rs`

The duplicated populate and status parsing that appears in both `find_all_documents` and
`find_document_by_id` moves into small typed helpers:

```rust
// infrastructure/http/handlers/content/params.rs

/// Parses "published" | "draft" → DocumentStatus
pub fn parse_status(s: &str) -> Result<DocumentStatus, ApiError>;

/// Validates field name strings → Vec<AttributeId>
/// Supports the `populate=*` wildcard by expanding to all owning relations.
pub fn parse_populate(
    fields: Option<HashSet<String>>,
    document_type: &DocumentType,
) -> Result<Option<Vec<AttributeId>>, ApiError>;

/// Looks up DocumentType by plural/singular API ID
pub fn resolve_document_type<S: AppState>(
    state: &S,
    api_type: &str,
) -> Result<&'static DocumentType, ApiError>;
```

**Implemented**

`infrastructure/http/handlers/content/params.rs` now hosts `parse_status`,
`parse_populate`, and `resolve_document_type`. `parse_populate` expands the
`populate=*` token to every owning relation declared on the document type
(Phase 5 wildcard requirement folded in here). All five content handlers
(`find_document_by_id`, `find_all_documents`, `create_new_document`,
`delete_existing_document`, `modify_relations`) call the helpers instead of
inlining the lookup/parse logic, and the previously duplicated
`DocumentTypeApiId::from_str` + `document_types().lookup()` chain is gone from
`mod.rs`.

### 4.3 — Move JSON → Command conversion to `request.rs`

`request.rs` constructs typed `Command` structs from HTTP inputs.
The existing `build_fields_from_payload` logic moves here but now delegates
to `ContentValue::from_json(value, field)` (Phase 1.4) instead of owning its own
type dispatch. Handlers never see raw `serde_json::Value` from user input:

```rust
// infrastructure/http/handlers/content/request.rs

pub fn parse_create_command(
    document_type: &'static DocumentType,
    payload: &serde_json::Value,
    user_id: Option<UserId>,
) -> Result<CreateDocumentCommand, ApiError>;

pub fn parse_update_command(
    document_type: &'static DocumentType,
    document_id: DocumentInstanceId,
    payload: &serde_json::Value,
    user_id: Option<UserId>,
) -> Result<UpdateDocumentCommand, ApiError>;

pub fn parse_modify_relations_command(
    document_type: &'static DocumentType,
    document_id: DocumentInstanceId,
    payload: &serde_json::Value,
) -> Result<ModifyRelationsCommand, ApiError>;
```

***Implemented***

### 4.4 — Fix camelCase field keys in `response.rs`

Dynamic field keys in `DocumentInstanceResponse` are currently emitted as snake_case
(`k.as_ref().to_owned()`). The architecture requires camelCase for all API responses.
Add a conversion applied to every dynamic key before it enters the response map:

```rust
// infrastructure/http/handlers/content/response.rs

fn to_api_key(snake: &str) -> String {
    // "first_name" → "firstName"
    let mut result = String::with_capacity(snake.len());
    let mut next_upper = false;
    for c in snake.chars() {
        if c == '_' { next_upper = true; }
        else if next_upper { result.extend(c.to_uppercase()); next_upper = false; }
        else { result.push(c); }
    }
    result
}
```

Also fix `DomainValue::Decimal → JsonValue` precision loss:
replace `num.to_f64().unwrap()` with `JsonValue::String(num.to_string())`.

### 4.5 — Thin handler bodies

After all the above, each handler is ~20 lines:

```rust
pub async fn find_all_documents<S: AppState>(
    State(state): State<S>,
    Path(api_type): Path<String>,
    QueryString(params): QueryString<QueryParams>,
) -> Result<ApiSuccess<ManyDocumentsResponse>, ApiError> {
    let document_type = resolve_document_type(&state, &api_type)?;
    let (page, page_size) = params.pagination_or_defaults();
    let cmd = FindDocumentsCommand {
        document_type,
        populate: parse_populate(params.populate, document_type)?,
        query: DocumentInstanceQuery::new()
            .paginate(page, page_size)
            .with_status(parse_status(&params.status)?),
    };
    let (documents, total) = state.documents_service().find(cmd).await?;
    Ok(ApiSuccess::new(StatusCode::OK, ManyDocumentsResponse::new(documents, page, page_size, total)))
}

pub async fn modify_relations<S: AppState>(
    State(state): State<S>,
    Path((api_type, id)): Path<(String, String)>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ApiError> {
    let document_type = resolve_document_type(&state, &api_type)?;
    let document_id = DocumentInstanceId::try_from(&id)?;
    let cmd = parse_modify_relations_command(document_type, document_id, &payload)?;
    state.documents_service().modify_relations(cmd).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

---

## Phase 5 — Wire Missing Features

Once the structure is clean, the following missing features each have an obvious single place
to implement and require no cross-cutting changes:

| Feature | Where to add |
|---|---|
| `filters` from query string | `params.rs` deserialization → `DocumentInstanceQuery` builder |
| `sort` from query string | Same as filters |
| `populate=*` wildcard | `parse_populate()` expands to `document_type.all_owning_relation_ids()` |
| `set` relation operation | `RelationOperation::Set` in commands + `apply_relation_ops` in `queries/relations.rs` |
| Correct pagination `total` | `find()` returns `(Vec, u64)` → `ManyDocumentsResponse::new` uses it |
| `update` document fields | `UpdateDocumentCommand` + `service.update()` + `repository.update()` |
| `publish` / `unpublish` | `PublishDocumentCommand` + service calls `instance.publish()` + `repository.update()` |
| Audit trail `user_id` | `CreateDocumentCommand` carries it → `DocumentInstance::new` stores it → `insert` writes it |

---

## Migration Order

Phases are ordered to avoid breaking the build at any intermediate step.
Each phase can be a separate PR.

| Phase | Description | Risk |
|---|---|---|
| **Phase 1** | Domain fixes | No API changes; pure domain improvement |
| **Phase 3 bugs only** | Fix the three correctness bugs (can be a hotfix PR) | No interface changes |
| **Phase 2** | Introduce `application/` module | New module; old infrastructure still compiles during transition |
| **Phase 3 rest** | Reorganise persistence into `queries/` + `mapping/` | Same interfaces, different file layout |
| **Phase 4** | Slim HTTP handlers | Handlers switch to new service API |
| **Phase 5** | Wire missing features | Additive only |

---

## What Each Boundary Enforces After the Refactor

| Layer | Input | Output | Knows about |
|---|---|---|---|
| **Domain** | — | — | Business rules, `DocumentInstance` lifecycle, field types, constraints |
| **Application** | `*Command` structs | `ServiceError` | Domain model + repository port |
| **HTTP handlers** | `axum::extract::*` | `axum::response::*` | `*Command` structs, HTTP status codes |
| **Persistence** | `DocumentType` + domain values | `DocumentInstance` | SQL, `PgRow`, `sea_query::Expr` |

No layer talks to a non-adjacent layer.
`DatabaseRowId` never leaves the persistence layer.
`RepositoryError` never leaves the application layer.
`serde_json::Value` from user input never enters the service layer raw.
