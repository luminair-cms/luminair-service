# Project description

Luminair is a Schema Driven CMS platform, like Strapi but focused on Speed and Reliability. Uses Domain-driven design and Hexagonal architecture where it is appropriate. It is based on microservices architecture.

For detailed information, see:
- [Architecture](../documentation/architecture.md)
- [Domain Model](../documentation/domain-model.md)
- [API Documentation](../documentation/api.md)
- [Schema Formats](../documentation/schemas.md)
- [Draft and Publishing Workflow](../documentation/draft-publish.md)

### Main principles

1. Using type-safe abstractions for Value types using `nutype` library.

2. Using `sqlx` for database interactions, leveraging its compile-time checked queries and async support.

2. Using `sea-query` for building type-safe SQL queries.

3. Implementing error handling using `anyhow` and `thiserror` for clear and maintainable error definitions.

4. Using trait-based design for extensibility and separation of concerns, following Rust's idiomatic patterns.

# Rust coding guidelines

1. Identify and Avoid Common Anti-Patterns

Using .clone() instead of borrowing — leads to unnecessary allocations.

Overusing .unwrap()/.expect() — causes panics and fragile error handling.

Calling .collect() too early — prevents lazy and efficient iteration.

Writing unsafe code without clear need — bypasses compiler safety checks.

Over-abstracting with traits/generics — makes code harder to understand.

Relying on global mutable state — breaks testability and thread safety.

Using macros that hide logic — makes code opaque and harder to debug.

Ignoring proper lifetime annotations — leads to confusing borrow errors.

Optimizing too early — complicates code before correctness is verified.

Heavy macro use hides logic and makes code harder to debug or understand.

2. Memory Safety Handling

Confirm how Rust's ownership model, borrowing rules, and lifetimes ensure memory safety.

Explore how reference-counted types like Rc, Arc, and Weak are used in code.

Include any common pitfalls (e.g., circular references) and how to avoid them.

Investigate the role of smart pointers (RefCell, Mutex, etc.) when sharing state between callbacks and signals.

### Used libraries
- `nutype` for defining newtypes with validation.
- `sqlx` for async database interactions with compile-time query checking.
- `sea-query` for building SQL queries in a type-safe way.
- `anyhow` and `thiserror` for error handling.
- `axum` for building the HTTP server and routing.
- `rust_decimal` for precise decimal arithmetic.
- `chrono` for date and time handling.

### Error handling

- `thiserror` simplifies the implementation of custom error type, removing boilerplates.

  Is ideal for library development where detailed information is beneficial for users (programmers).

- `anyhow` consolidates errors that implement std::error::Error.

  Is suited for applications where internal details are not crucial, providing simplified information to users.

- While `thiserror` provides detailed error information for specific reactions, `anyhow` hides internal details.

see: 
https://www.howtocodeit.com/guides/the-definitive-guide-to-rust-error-handling 
for detailed comparison and best practices.