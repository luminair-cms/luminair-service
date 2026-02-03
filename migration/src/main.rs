use crate::{
    domain::migration::Migration,
    infrastructure::{persistence::PersistenceAdapter, settings::Settings},
};
use luminair_common::{database, load_documents};

pub mod domain;
pub mod infrastructure;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::from_env()?;

    let documents = load_documents(&settings.schema_config_path)?;
    println!("Configuration loaded");

    let database = database::connect(&settings.database).await?;
    println!("Connected to DB");
    let persistence = PersistenceAdapter::new(database);

    // migrate database schema conform documents configuration
    let migration = Migration::new(documents, persistence);
    migration.migrate().await?;
    println!("Configuration migrated");

    Ok(())
}
