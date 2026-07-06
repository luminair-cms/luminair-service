use crate::domain::migration::{
    MigrationStep, MigrationStepItem, documents_into_tables, plan_migration,
};
use crate::domain::tables::Table;
use luminair_common::DocumentTypesRegistry;
use std::future::Future;

pub trait Persistence: Send + Sync + Clone + 'static {
    /// load persistence from database
    fn load(&self) -> impl Future<Output = Result<Vec<Table>, anyhow::Error>>;
    /// apply migration steps to database
    fn apply_migration_steps(
        &self,
        steps: Vec<MigrationStepItem>,
    ) -> impl Future<Output = Result<(), anyhow::Error>>;
    /// extract database schema
    fn database_schema(&self) -> &str;
}

#[derive(Clone)]
pub struct Migration<P: Persistence> {
    documents: &'static dyn DocumentTypesRegistry,
    persistence: P,
}

impl<P: Persistence> Migration<P> {
    pub fn new(documents: &'static dyn DocumentTypesRegistry, persistence: P) -> Self {
        Self {
            documents,
            persistence,
        }
    }

    /// migrate database schema conform documents configuration
    pub async fn migrate(&self, dry_run: bool) -> Result<(), anyhow::Error> {
        let needed_schema = documents_into_tables(self.documents);
        let actual_schema = self.persistence.load().await?;

        let steps = plan_migration(
            &needed_schema,
            &actual_schema,
            self.persistence.database_schema(),
        )?;

        if dry_run {
            println!("--- DRY-RUN: The following SQL DDL would be executed ---");
            if steps.is_empty() {
                println!("No migration steps needed. Database schema is up to date.");
            } else {
                for step in &steps {
                    println!("-- Context: {}", step.ctx());
                    for ddl in step.clone().ddls() {
                        println!("{};", ddl);
                    }
                }
            }
            return Ok(());
        }

        self.persistence.apply_migration_steps(steps).await?;

        Ok(())
    }
}
