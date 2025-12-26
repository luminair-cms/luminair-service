use chrono::{DateTime, Utc};
use luminair_common::infrastructure::database::Database;
use sqlx::postgres::PgRow;

use crate::domain::{Persistence, ResultRow, ResultSet};

#[derive(Clone, Debug)]
pub struct PersistenceAdapter {
    database: &'static Database,
}

pub struct ResultSetImpl {
    rows: Vec<ResultRow>
}

impl ResultSet for ResultSetImpl {
    fn into_rows(self) -> Vec<ResultRow> {
        self.rows
    }
}

impl TryFrom <PgRow> for ResultRow {
    type Error = anyhow::Error;

    fn try_from(value: PgRow) -> Result<Self, Self::Error> {
        use sqlx::Row;
        
        let document_id: i32 = value.try_get("document_id")?;
        let created_at: DateTime<Utc> = value.try_get("created_at")?;
        let updated_at: DateTime<Utc> = value.try_get("updated_at")?;
        
        Ok(ResultRow { document_id, created_at, updated_at })
    }
}

impl PersistenceAdapter {
    pub fn new(database: &'static Database) -> Self {
        Self { database }
    }
}

impl Persistence for PersistenceAdapter {
    async fn select_all(&self, query: &crate::domain::Query) -> Result<impl ResultSet, anyhow::Error> {
        let sql = query.generate_select();
        let mut db_rows = sqlx::query(&sql).fetch(self.database.database_pool());
        
        let mut rows = Vec::new();
        
        use futures::TryStreamExt;
        while let Some(row) = db_rows.try_next().await? {
            let result_row = ResultRow::try_from(row)?;
            rows.push(result_row);
        }
        
        Ok(ResultSetImpl { rows })
    }
}
