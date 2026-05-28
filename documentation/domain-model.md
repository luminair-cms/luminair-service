# Schema Domain Model

The `common` crate defines the schema domain for Luminair. It models document types, field metadata, relations, and validation constraints as first-class domain concepts.

## Core concepts

- `DocumentType`: a schema definition that describes a document kind, its fields, relations, and options.
- `DocumentField`: a typed field inside a document schema.
- `DocumentRelation`: a relation from one document type to another.
- `FieldType`: the allowed data types for schema fields.
- `FieldConstraint`: additional validation rules for a field.
- `DocumentTypeId`, `DocumentTypeApiId`, `AttributeId`: strongly typed identifiers used throughout the schema.

## DocumentType

`DocumentType` is the root schema entity. It contains:

- `id: DocumentTypeId` — internal schema identity.
- `kind: DocumentKind` — either `Collection` or `SingleType`.
- `info: DocumentTypeInfo` — title, singular/plural names, and description.
- `options: Option<DocumentTypeOptions>` — additional schema options such as draft-and-publish and localizations.
- `fields: HashSet<DocumentField>` — typed document fields.
- `relations: HashSet<DocumentRelation>` — links to other document types.

The schema is intentionally immutable after load, and document types are compared by their `id`.

## DocumentTypeInfo and options

`DocumentTypeInfo` carries descriptive metadata:

- `title`: human-friendly name.
- `singular_name`: API name for singletons or single items.
- `plural_name`: API name for collections.
- `description`: optional documentation string.

`DocumentTypeOptions` includes:

- `draft_and_publish`: whether the document type supports draft/publish workflow.
- `localizations`: a list of enabled localization IDs.

## DocumentField

A `DocumentField` represents a single typed attribute of a document type.

Fields include:

- `id: AttributeId` — unique field identifier.
- `field_type: FieldType` — scalar or structured data type.
- `unique: bool` — whether values must be unique.
- `required: bool` — whether values must be present.
- `constraints: HashSet<FieldConstraint>` — additional validation rules.

`DocumentType` provides helper methods such as `ordered_fields()` to produce a stable sort order.

## DocumentRelation

A `DocumentRelation` models a link between document types.

It includes:

- `id: AttributeId` — relation identifier.
- `relation_type: RelationType` — one of `HasOne`, `HasMany`, `BelongsToOne`, or `BelongsToMany`.
- `target: DocumentTypeId` — the related document type.

***Relation type ManyToMany moved out of MVP***

This is because of `ManyToMany` requires shared owning model, especially in case of DraftAndPublish mode.

Reduced implementation of ManyToMany modeled after combination of `HasMany` + `BelongsToMany` and assumes management of connection ONLY from owning side.

## FieldType

`FieldType` defines allowed field data kinds:

- `Uid`
- `Uuid`
- `Text`
- `LocalizedText`
- `Integer(IntegerSize)`
- `Decimal { precision, scale }`
- `Date`
- `DateTime`
- `Boolean`
- `Json`

The type system is used both for schema validation and for guiding database mapping and UI generation.

## FieldConstraint

`FieldConstraint` encodes field-level validation rules:

- `Pattern(String)`
- `MinimalLength(usize)`
- `MaximalLength(usize)`
- `MinimalIntegerValue(i32)`
- `MaximalIntegerValue(i32)`

`FieldConstraint::is_applicable_for()` checks whether a constraint makes sense for a given `FieldType`.
This validation is performed during schema loading to prevent invalid combinations such as text patterns on integer fields.

## Value objects and IDs

The schema domain uses newtypes for identifier safety:

- `DocumentTypeId` — validated document type identifier.
- `DocumentTypeApiId` — validated public API ID.
- `AttributeId` — validated field/relation identifier.

These types enforce normalization and eligibility rules at construction, reducing accidental misuse of raw strings.

## Schema registry

The `common` crate defines the `DocumentTypesRegistry` trait:

- `iterate()` — iterate all document metadata.
- `get()` — lookup a document type by internal ID.
- `lookup()` — lookup by API ID.

This registry allows the migration and service layers to consume schema metadata in a read-only, thread-safe way.

## Invariants and best practices

- Schema validation happens at load time, not at runtime request processing.
- Field constraints are validated against field types before a `DocumentType` is built.
- Relations are explicit and strongly typed.
- Document types are uniquely identified by `DocumentTypeId` and compared by identity.

The common crate exposes schema metadata as domain logic, while runtime behavior and persistence are implemented in the `service` and `migration` crates.

## Service crate domain model

The `service` crate builds on the `common` schema domain and adds runtime document lifecycle, content values, audit metadata, and the API-facing state model.

### DocumentInstance

A `DocumentInstance` is a runtime entity representing one stored document record.

It contains:

- `id: DatabaseRowId` — internal database row key.
- `document_id: DocumentInstanceId` — stable UUID for the document instance.
- `content: DocumentContent` — typed field values and publication state.
- `relations: HashMap<AttributeId, Vec<DocumentRelation>>` — resolved relations to other documents.
- `audit: AuditTrail` — system metadata for creation/update and version tracking.

`DocumentInstance` is the service-side aggregate root for a document record and is used by application services and repositories.

### DocumentContent and values

`DocumentContent` stores the actual document payload:

- `fields: HashMap<AttributeId, ContentValue>` — field values keyed by attribute ID.
- `publication_state: PublicationState` — current draft/published state.

`ContentValue` is a small domain-union representing content values:

- `Scalar(DomainValue)` — scalar typed values.
- `LocalizedText(HashMap<String, String>)` — locale-specific text mapping.
- `Null` — explicit missing value.

`DomainValue` is the concrete typed domain value model:

- `Text(String)`
- `Integer(i64)`
- `Decimal(Decimal)`
- `Boolean(bool)`
- `Date(NaiveDate)`
- `DateTime(DateTime<Utc>)`
- `Email(Email)`
- `Url(Url)`
- `Uuid(Uuid)`
- `Json(HashMap<String, String>)`

This design keeps the runtime document model separate from schema metadata, while still supporting the types needed by the service layer.

### Publication workflow

Publication is modeled in the service crate by `PublicationState`:

- `Draft { revision: i32 }`
- `Published { revision: i32, published_at: DateTime<Utc>, published_by: Option<UserId> }`

`revision` and `AuditTrail.version` are **independent counters** serving different purposes:

| Counter | Increments on | Meaning |
|---------|---------------|---------|
| `AuditTrail.version` | every save (edit, publish, unpublish) | how many times the document was modified |
| `PublicationState.revision` | publish only | which publication of this document this is |

Semantics and initial values:

- `DocumentContent::new` initialises `Draft { revision: 0 }`. `revision = 0` means the document has never been published.
- `DocumentInstance::new` sets `AuditTrail.version = 1` (first save).
- Editing increments `AuditTrail.version` only; `revision` is unchanged.
- Publishing increments `revision` from its current Draft value (`0 → 1` on first publish, `N → N+1` on subsequent publishes) and independently increments `AuditTrail.version` because publish is also a save.
- After publishing, further edits return the document to `Draft { revision: N }` where `N` is the last published revision. `revision` is frozen at `N` until the next publish.

### AuditTrail and system metadata

`AuditTrail` captures infrastructure-level metadata:

- `created_at`, `created_by`
- `updated_at`, `updated_by`
- `version`

This is explicitly separate from `PublicationState` so domain publication semantics remain distinct from audit/version tracking.

### Application and repository contracts

The service crate defines reusable traits for runtime behavior:

- `AppState` exposes shared application state and the document services implementation.
- `DocumentServices` is the service-layer contract for document operations.
- Repository query types and traits support persistence abstraction.

### How the service model uses the common schema

The service layer consumes `DocumentTypesRegistry` from the `common` crate.

- `AppState` exposes a reference to the registry.
- Document services use the registry to interpret schema metadata for validation, query behavior, and API serialization.

This separation keeps `common` focused on schema metadata, while `service` handles runtime document lifecycle, persistence, and user-facing behavior.