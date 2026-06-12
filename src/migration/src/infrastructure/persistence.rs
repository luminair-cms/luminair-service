use std::collections::HashSet;

use luminair_common::database::Database;
use anyhow::Context;
use sqlx::Executor;

use crate::application::Persistence;

#[derive(Clone)]
pub struct PersistenceAdapter {
    database: &'static Database,
}

impl PersistenceAdapter {
    pub fn new(database: &'static Database) -> Self {
        Self { database }
    }
}

impl Persistence for PersistenceAdapter {
    async fn load(&self) -> Result<HashSet<String>, anyhow::Error> {
        let sql = "SELECT table_name
            FROM information_schema.tables
            WHERE
              table_schema = $1
              AND table_type = 'BASE TABLE'
              AND table_name != 'geometry_columns'
              AND table_name != 'spatial_ref_sys'";
        
        let mut rows = sqlx::query_scalar::<_, String>(sql)
            .bind(self.database.database_schema())
            .fetch(self.database.database_pool());
        
        let mut set = HashSet::new();
        
        use futures::TryStreamExt;
        while let Some(name) = rows.try_next().await? {
            set.insert(name);
        }
        
        Ok(set)
    }

    async fn apply_migration_steps(&self, steps: Vec<impl crate::domain::migration::MigrationStep>)-> Result<(), anyhow::Error> {
        use futures::stream::{self, StreamExt};
    
        let mut stream = stream::iter(steps);
        while let Some(step) = stream.next().await {
            let ctx = step.ctx();
            let ddls = step.ddls();
            execute_in_transaction(self.database, ddls, ctx).await?;
        }
    
        Ok(())
    }
    
    fn database_schema(&self) -> &str {
        self.database.database_schema()
    }
}

async fn execute_in_transaction(
    database: &luminair_common::database::Database,
    queries: Vec<String>,
    ctx: &'static str,
) -> Result<(), anyhow::Error> {
    let mut transaction = database
        .database_pool()
        .begin()
        .await
        .context(format!("failed to start {} transaction", ctx))?;

    for ddl in queries {
        let query = sqlx::AssertSqlSafe(ddl);
        transaction
            .execute(query)
            .await
            .context(format!("failed to execute {} query", ctx))?;
    }

    transaction
        .commit()
        .await
        .context(format!("failed to commit {} transaction", ctx))?;

    Ok(())
}