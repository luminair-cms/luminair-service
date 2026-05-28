use luminair_common::DocumentTypesRegistry;
use uuid::ContextV7;

pub mod builders;
pub mod repository;
pub mod mapping;

const CLOCK_SEQUENCE: ContextV7 = ContextV7::new();
