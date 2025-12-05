use std::collections::HashSet;

use luminair_common::infrastructure::database::Database;

use crate::domain::tables::Tables;


#[derive(Clone)]
pub struct TablesAdapter {
    database: Database
}

impl TablesAdapter {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

impl Tables for TablesAdapter {
    
    // https://github.com/strapi/strapi/blob/develop/packages/core/database/src/dialects/postgresql/schema-inspector.ts
    
    async fn load(&self) -> Result<HashSet<String>, anyhow::Error> {
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
}
