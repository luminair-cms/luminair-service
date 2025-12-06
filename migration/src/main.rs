use luminair_common::{infrastructure::database::Database, load_documents};

use crate::{
    domain::migration::Migration,
    infrastructure::{persistence::PersistenceAdapter, settings::Settings},
};

pub mod domain;
pub mod infrastructure;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::from_env()?;

    let documents = load_documents(&settings.schema_config_path)?;
    println!("Configuration loaded");

    let database = Database::new(&settings.database).await?;
    println!("Connected to DB");

    let persistence = PersistenceAdapter::new(database.clone());

    let migration = Migration::new(documents, persistence);
    migration.migrate().await?;
    println!("Configuration migrated");

    Ok(())
}
