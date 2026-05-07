# Project description

Luminair is a Schema Driven CMS platform, like Strapi but focused on Speed and Reliability. Uses Domain-driven design and Hexagonal architecture where it is appropriate. It is based on microservices architecture, separated parts:

### Crates:

*Common*

Contains Documents schema and database infrastructure

*Migration*

Migration CLI, uses Schema Registry for tables creation, deletion etc. Needs DB privileges for DDL

*Service*

Microservice, uses Schema Registry, provides dynamic API for Schema Metadata and Documents manipulation. Needs privileges only for DML

### Main principles

1. Using type-safe abstractions for Value types using `nutype` library.

2. Using `sea-orm` for database interactions, leveraging its features for efficient and safe data access.

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