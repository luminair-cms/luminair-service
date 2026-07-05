use migration::{
    application::Migration,
    infrastructure::{persistence::PersistenceAdapter, settings::Settings},
};
use luminair_common::{database, load_documents};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::from_env()?;

    let args: Vec<String> = std::env::args().collect();
    let is_check = args.contains(&"--check".to_string()) || args.contains(&"-c".to_string());
    let is_dry_run = args.contains(&"--dry-run".to_string()) || args.contains(&"-d".to_string());

    if is_check {
        println!("Checking document configuration validity...");
        let documents = load_documents(&settings.schema_config_path)?;
        let mut has_error = false;
        for doc in documents.iterate() {
            for relation in &doc.relations {
                if documents.get(&relation.target).is_none() {
                    eprintln!(
                        "Error: Relation '{}' in document type '{}' targets unknown document type '{}'",
                        relation.id, doc.id, relation.target
                    );
                    has_error = true;
                }
            }
        }
        if has_error {
            anyhow::bail!("Documents configuration is invalid.");
        } else {
            println!("Configuration is valid.");
            return Ok(());
        }
    }

    let documents = load_documents(&settings.schema_config_path)?;
    println!("Configuration loaded");

    let database = database::connect(&settings.database).await?;
    println!("Connected to DB");
    let persistence = PersistenceAdapter::new(
        database.database_pool().clone(),
        database.database_schema(),
    );

    // migrate database schema conform documents configuration
    let migration = Migration::new(documents, persistence);
    migration.migrate(is_dry_run).await?;

    if is_dry_run {
        println!("Dry-run migration complete (no changes applied)");
    } else {
        println!("Configuration migrated");
    }

    Ok(())
}
