# Project rules for AI agents

This file is the single source of truth for all AI agents (including Antigravity, Claude, Gemini, and GitHub Copilot) working in the Luminair repository.

---

## Project Description

Luminair is a Schema-Driven CMS platform (similar to Strapi) focused on Speed and Reliability. It uses Domain-driven design (DDD) and Hexagonal architecture where appropriate, built on a microservices-inspired architecture.

### Reference Documentation
- [Architecture](../documentation/architecture.md)
- [Domain Model](../documentation/domain-model.md)
- [API Documentation](https://github.com/luminair-cms/documentation/blob/main/api.md)
- [Schema Formats](../documentation/schemas.md)
- [Database Structure](../documentation/database.md)
- [Draft & Publish Workflow](https://github.com/luminair-cms/documentation/blob/main/draft-publish.md)
- [Draft & Publish Database Design](../documentation/draft-publish.md)

> [!NOTE]
> Reference documentation links starting with `https://github.com/luminair-cms/documentation/blob/main/` resolve to the local sibling directory `../documentation/`. AI agents should search and read these files from the local folder.

> [!IMPORTANT]
> If these instructions conflict with the actual codebase, the code is the source of truth. Flag any discrepancy you notice.

---

## Core Principles

1. **Type-Safe Value Types**: Use type-safe abstractions for value types using the `nutype` library.
2. **Compile-Time Checked SQL**: Use `sqlx` for database interactions, leveraging compile-time query verification and async support.
3. **Type-Safe SQL Building**: Use `sea-query` for constructing dynamic, type-safe SQL queries.
4. **Structured Error Handling**: Implement error handling using `anyhow` (for application/orchestration errors) and `thiserror` (for domain-specific errors).
5. **Trait-Based Design**: Leverage trait-based design for extensibility, testability, and separation of concerns.

---

## Rust Coding Guidelines

### 1. Avoid Common Anti-Patterns
* **Unnecessary Cloning**: Avoid `.clone()` unless ownership transfer is strictly required; prefer borrowing.
* **Fragile Panics**: Avoid `.unwrap()` and `.expect()` in production code. Propagate errors safely using `Result`.
* **Eager Collection**: Do not call `.collect()` too early on iterators; keep them lazy and efficient.
* **Unsafe Code**: Do not write `unsafe` blocks unless absolutely necessary and documented.
* **Over-Abstraction**: Avoid over-abstracting with complex traits/generics. Keep types straightforward.
* **Global Mutable State**: Rely on dependency injection via app state rather than global mutable static variables.
* **Opaque Logic**: Avoid heavy macro use that hides implementation details and makes debugging difficult.
* **Lifetime Pitfalls**: Handle lifetime annotations carefully to avoid confusing compiler borrow errors.

### 2. Memory Safety & Sharing
* Adhere to Rust's ownership model, borrowing rules, and lifetimes.
* Use reference-counted types (`Rc`, `Arc`, `Weak`) only where multiple ownership is necessary.
* Avoid circular references when using smart pointers (or break them using `Weak` references).
* Use thread-safe interior mutability (`Mutex`, `RwLock`) or message passing (`mpsc` channels) when sharing state between threads.

---

## Library & Error Conventions

### Used Libraries
* `nutype`: validation and sanitization of newtypes.
* `sqlx`: async database pool and query checking.
* `sea-query`: dynamic SQL query construction.
* `axum`: HTTP server and routing layer.
* `rust_decimal`: high-precision decimal operations.
* `chrono`: datetime operations.

### Error Handling Philosophy
* Use `thiserror` for domain and library modules where detailed, structured errors are beneficial for recovery or programmatic handling.
* Use `anyhow` for top-level application code, orchestrators, and scripts where details are aggregated/logged rather than handled dynamically.
* Refer to [Rust Error Handling Best Practices](https://www.howtocodeit.com/guides/the-definitive-guide-to-rust-error-handling) for detailed comparisons.

---

## Code & File Style

* **Entity Ordering**: In Rust source files:
  1. `pub` entities first, `private` entities last.
  2. Common/general entities first, specialized/helper entities last.
* **Documentation Guidelines**:
  * Document all public structs, enums, traits, and functions using docstrings (`///`).
  * Keep inline documentation synchronized with code changes.
* **Code Formatting**:
  * Before committing changes, you must check formatting by running:
    ```bash
    cargo fmt --all --check
    ```
* **Dependency Sorting & Management**: In Cargo manifests (`Cargo.toml`):
  1. All dependencies under `[dependencies]` and `[dev-dependencies]` must be sorted alphabetically.
  2. Local workspace dependencies (e.g. `luminair_common = { path = ... }`) should be placed first, separated by a blank line from third-party/external dependencies.
  3. Shared third-party dependency versions must be centralized in the workspace root `Cargo.toml` under `[workspace.dependencies]`, and inherited in sub-crates using `{ workspace = true }`.

---

## Command Cheat Sheet

### Build & Compilation
```bash
cargo build
cargo check --workspace
cargo fmt --all --check            # Verify code formatting
```

### Running Tests
Make sure Docker is running (needed by containerized `testcontainers` integration tests).
```bash
cargo test                         # Run all tests
cargo test --lib                   # Run unit tests only
cargo test --package migration     # Run tests in migration crate
```

### Database Migrations
```bash
cargo run --package migration               # Execute standard migration
cargo run --package migration -- --check    # Check schema configuration validity
cargo run --package migration -- --dry-run  # Dry-run and print DDLs
```
