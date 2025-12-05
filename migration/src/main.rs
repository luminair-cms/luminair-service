use luminair_common::infrastructure::{adapters::documents::DocumentsAdapter, database::Database};

use crate::{
    domain::migration::Migration,
    infrastructure::{migration::MigrationAdapter, settings::Settings, tables::TablesAdapter},
};

pub mod domain;
pub mod infrastructure;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::from_env()?;

    let documents = DocumentsAdapter::load(&settings.schema_config_path)?;
    println!("Configuration loaded");

    let database = Database::new(&settings.database).await?;
    println!("Connected to DB");

    let tables = TablesAdapter::new(database.clone());

    let migration = MigrationAdapter::new(documents, tables, database);
    migration.migrate().await?;
    println!("Configuration migrated");

    Ok(())
}
