# Architecture

This document describes the backend architecture of the Luminair service and the responsibilities of each crate. It is intentionally focused on structure, runtime boundaries, and how schema-driven metadata is used in the system.

## High-level architecture

Luminair is built as a small Rust-based backend platform with explicit separation between:

- shared domain/schema code,
- migration tooling for DDL,
- service runtime for DML and dynamic API handling.

The system is schema-driven: document metadata and JSON schema definitions are the source of truth for both table generation and runtime behavior.

## Crate responsibilities

### `common`
- Shared domain model and type definitions.
- Schema registry interfaces and document metadata.
- Common persistence abstractions used by both `migration` and `service`.

### `migration`
- CLI tool for database schema creation and migration.
- Uses the schema registry to derive tables and DDL statements.
- Requires database privileges for DDL operations only.

### `service`
- HTTP microservice exposing document and schema APIs.
- Uses the schema registry to provide dynamic API behavior for schema metadata and documents.
- Operates with DML privileges only.

## Runtime architecture

### Service startup
- Loads configuration from `config/default.yaml` and environment.
- Initializes application state implementing `AppState`.
- Exposes HTTP routes via `axum`.
- Uses `sqlx` and `sea-query` for database access.

### Migration runtime
- Reads the same schema registry from `common`.
- Generates or updates database tables using schema metadata.
- Keeps migration logic separate from request handling.

## Domain vs infrastructure separation

### Domain layer
- Core document abstractions are defined in the `service/src/domain` module.
- `DocumentInstance` represents an instance of a document type.
- `DocumentContent` contains schema-driven fields and `PublicationState`.
- `PublicationState` models draft/published workflow and revision tracking.

### Infrastructure layer
- `AuditTrail` records system metadata such as creation/update timestamps, user IDs, and version.
- Repository traits in `service/src/domain/repository` abstract persistence behind interfaces.
- The service layer uses trait-based design for extensibility and testability.

## Publication and versioning

Publication is modeled as two separate concerns:

- `PublicationState` is domain state for document content lifecycle.
- `AuditTrail` is infrastructure metadata for system auditing.

This allows the system to track both the publication workflow and the overall history of changes.

## Schema-driven design

The backend is driven by JSON schema definitions under `config/schema/`.
These definitions are used to:

- build document types,
- generate database structures,
- validate document instances,
- drive the dynamic API.

## Naming conventions

This project follows clear naming conventions across layers to avoid ambiguity:

- Rust code and internal domain identifiers: use snake_case (for example `document_type_id`, `draft_and_publish`).
- Postgres database identifiers (tables and columns): use snake_case (for example `document_id`, `published_at`).
- JSON schema files under `config/schema/` and API payloads: use camelCase (for example `documentId`, `draftAndPublish`, `publishedAt`).

When documenting fields, examples will show both forms where applicable, for example:

- API/JSON: `"publishedAt"`
- DB/Rust: `published_at`

The mapping between representations is deterministic: camelCase keys in schema and API are mapped to snake_case identifiers in the Rust domain and database.

## Service crate — planned refactoring

The `service` crate has a known set of structural issues and correctness bugs that are tracked
in a dedicated document. Any work on the `service` crate should be done in accordance with
that plan.

See: [Service Refactoring Plan](service-refactoring-plan.md)

## Best-practice guidance for AI/agent use

- Keep architecture documentation declarative and sectioned.
- Link into specialized docs rather than embedding every detail.
- Use this file for structure and boundaries, and reference:
  - `documentation/domain-model.md`
  - `documentation/api.md`
  - `documentation/schemas.md`
  - `documentation/draft-publish.md`
  - `documentation/service-refactoring-plan.md`
