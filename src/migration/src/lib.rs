//! Library interface for the `migration` crate.
//!
//! Exposes the application, domain, and infrastructure modules so that
//! integration tests in `tests/` can reference them.

pub mod application;
pub mod domain;
pub mod infrastructure;
