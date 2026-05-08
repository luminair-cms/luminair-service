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

## Best-practice guidance for AI/agent use

- Keep architecture documentation declarative and sectioned.
- Link into specialized docs rather than embedding every detail.
- Use this file for structure and boundaries, and reference:
  - `documentation/domain-model.md`
  - `documentation/api.md`
  - `documentation/schemas.md`
  - `documentation/draft-publish.md`
