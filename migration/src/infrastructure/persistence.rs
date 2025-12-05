use std::collections::HashSet;

use luminair_common::infrastructure::database::Database;

use crate::domain::persistence::Persistence;

#[derive(Clone)]
pub struct PersistenceAdapter {
    database: Database,
}

impl PersistenceAdapter {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

impl Persistence for PersistenceAdapter {
    async fn load(&self) -> Result<std::collections::HashSet<String>, anyhow::Error> {
        use futures::TryStreamExt;
        
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
            self.database.excute_in_transaction(ddls, ctx).await?;
        }
    
        Ok(())
    }
    
    fn datbase_schema(&self) -> &str {
        self.database.database_schema()
    }
}