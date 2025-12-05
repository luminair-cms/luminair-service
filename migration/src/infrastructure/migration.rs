use luminair_common::infrastructure::{adapters::documents::DocumentsAdapter, database::Database};

use crate::{domain::migration::{Migration, apply_migration_steps, migration_steps}, infrastructure::tables::TablesAdapter};


#[derive(Clone)]
pub struct MigrationAdapter {
    documents: DocumentsAdapter,
    tables: TablesAdapter,
    database: Database,
}

impl MigrationAdapter {
    pub fn new(
        documents: DocumentsAdapter,
        tables: TablesAdapter,
        database: Database,
    ) -> Self {
        Self {
            documents,
            tables,
            database,
        }
    }

    // working with SERIAL types: https://www.bytebase.com/reference/postgres/how-to/how-to-use-serial-postgres/
}

impl Migration for MigrationAdapter {
    type D = DocumentsAdapter;
    type T = TablesAdapter;
    
    async fn migrate(&self) -> Result<(), anyhow::Error> {
        let databse_schema = self.database.database_schema();
        let steps = migration_steps(databse_schema, &self.documents, &self.tables).await?;
        apply_migration_steps(steps, &self.database).await?;
        Ok(())
    }
}
