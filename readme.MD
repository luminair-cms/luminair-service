# Luminair Backend

Luminair is a schema-driven CMS backend written in Rust. This repository contains the backend services for the Luminair platform, with a focus on performance, reliability, and a Strapi-like REST API.

## Overview

- `common`: shared document schema and database infrastructure
- `migration`: migration CLI for schema registry and DDL operations
- `service`: HTTP microservice exposing a dynamic API for content metadata and documents

This project is early stage and is primarily built as a Rust learning platform using domain-driven design and hexagonal architecture.

## Documentation

- [REST API](documentation/api.md)
- [Architecture](documentation/architecture.md)
- [Domain model](documentation/domain-model.md)
- [Schema definitions](documentation/schemas.md)
- [Draft & publish workflow](documentation/draft-publish.md)
- [Database design](documentation/database.md)

## Architecture notes

- The service is split into separate crates for core domain logic, migration tooling, and runtime HTTP handling.
- The migration tool requires DDL privileges to create and maintain database tables.
- The service itself is intended to run with DML privileges only.
- A separate UI repository and schema generation tool are planned for future work.

## Key design principles

- Schema-driven content modeling
- Field-level i18n support
- Separation between domain logic and infrastructure
- Minimal REST API conventions inspired by Strapi

## Getting started

For API usage and examples, see [documentation/api.md](documentation/api.md).

## Testing

Luminair contains unit tests and containerized integration tests. To run tests, make sure Docker is running on your machine (required by the `testcontainers` integration tests).

### Run all tests
```bash
cargo test
```

### Run unit tests only
To run unit tests across all packages (excluding integration tests):
```bash
cargo test --lib
```

To run unit tests specifically for the `migration` crate:
```bash
cargo test --package migration --lib
```

### Run integration tests only
To run all integration tests:
```bash
cargo test --test integration_tests
```

To run integration tests specifically for the `migration` crate:
```bash
cargo test --package migration --test integration_tests
```

