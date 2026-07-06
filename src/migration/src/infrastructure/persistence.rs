use crate::domain::tables::{ForeignKeyConstraint, Table};
use anyhow::Context;
use sqlx::{Executor, PgPool};

use crate::application::Persistence;
use crate::domain::migration::MigrationStep;

#[derive(Clone)]
pub struct PersistenceAdapter {
    pool: PgPool,
    schema: String,
}

impl PersistenceAdapter {
    pub fn new(pool: PgPool, schema: impl Into<String>) -> Self {
        Self {
            pool,
            schema: schema.into(),
        }
    }
}

impl Persistence for PersistenceAdapter {
    async fn load(&self) -> Result<Vec<Table>, anyhow::Error> {
        let tables_sql = "SELECT table_name
            FROM information_schema.tables
            WHERE
              table_schema = $1
              AND table_type = 'BASE TABLE'
              AND table_name != 'geometry_columns'
              AND table_name != 'spatial_ref_sys'";

        let table_names = sqlx::query_scalar::<_, String>(tables_sql)
            .bind(&self.schema)
            .fetch_all(&self.pool)
            .await?;

        let mut tables_map = std::collections::HashMap::new();
        for name in table_names {
            tables_map.insert(name.clone(), Table::new(name, vec![], vec![], vec![]));
        }

        let fkeys_sql = "SELECT
            tc.table_name,
            kcu.column_name,
            ccu.table_name AS referenced_table_name,
            ccu.column_name AS referenced_column_name
        FROM
            information_schema.table_constraints AS tc
            JOIN information_schema.key_column_usage AS kcu
              ON tc.constraint_name = kcu.constraint_name
              AND tc.table_schema = kcu.table_schema
            JOIN information_schema.constraint_column_usage AS ccu
              ON ccu.constraint_name = tc.constraint_name
              AND ccu.table_schema = tc.table_schema
        WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_schema = $1";

        let fk_rows = sqlx::query_as::<_, (String, String, String, String)>(fkeys_sql)
            .bind(&self.schema)
            .fetch_all(&self.pool)
            .await?;

        for (table_name, column_name, ref_table, ref_col) in fk_rows {
            if let Some(table) = tables_map.get_mut(&table_name) {
                table.foreign_keys.push(ForeignKeyConstraint::new(
                    table_name,
                    column_name,
                    ref_table,
                    ref_col,
                ));
            }
        }

        Ok(tables_map.into_values().collect())
    }

    async fn apply_migration_steps(
        &self,
        steps: Vec<crate::domain::migration::MigrationStepItem>,
    ) -> Result<(), anyhow::Error> {
        use futures::stream::{self, StreamExt};

        let mut stream = stream::iter(steps);
        while let Some(step) = stream.next().await {
            let ctx = step.ctx();
            let ddls = step.clone().ddls();
            execute_in_transaction(&self.pool, ddls, ctx).await?;
        }

        Ok(())
    }

    fn database_schema(&self) -> &str {
        &self.schema
    }
}

async fn execute_in_transaction(
    pool: &PgPool,
    queries: Vec<String>,
    ctx: &'static str,
) -> Result<(), anyhow::Error> {
    let mut transaction = pool
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
