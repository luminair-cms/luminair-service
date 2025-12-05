use std::collections::HashSet;

use crate::domain::migration::MigrationStep;

pub trait Persistence: Send + Sync + Clone + 'static {
    /// load tables from database
    fn load(&self) -> impl Future<Output = Result<HashSet<String>, anyhow::Error>>;
    /// apply migration steps to database
    fn apply_migration_steps(&self, steps: Vec<impl MigrationStep>)-> impl Future<Output = Result<(), anyhow::Error>>;
    /// extract database schema
    fn datbase_schema(&self) -> &str;
}