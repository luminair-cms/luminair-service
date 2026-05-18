use luminair_common::DocumentTypesRegistry;
use uuid::ContextV7;

pub mod builders;
pub(crate) mod result;
pub mod params;
pub mod repository;

const CLOCK_SEQUENCE: ContextV7 = ContextV7::new();
