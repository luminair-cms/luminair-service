# Luminair Backend Service - Documentation

This directory contains backend-specific documentation for the `luminair-service` codebase.

## Backend Documentation Index

* **[Architecture](architecture.md)**: High-level overview of the backend domain-driven design, hexagonal architecture, and modular service/migration structure.
* **[Domain Model](domain-model.md)**: Specifications for service-side aggregates, repositories, newtypes (`nutype`), and error handling strategies.
* **[Schema Formats](schemas.md)**: JSON structure definitions for defining collection and single-type models.
* **[Database Design](database.md)**: Details on PostgreSQL schema generation, table-naming, draft and snapshot tables, and migration execution modes.
* **[Draft & Publish Database Design](draft-publish.md)**: Details on SQL DDL/DML transition operations and relation versioning.
