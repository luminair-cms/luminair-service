use std::collections::HashMap;
use chrono::{DateTime, Utc};
use luminair_common::infrastructure::database::Database;
use sqlx::postgres::PgRow;

use crate::domain::{Persistence, Query, ResultRow, ResultSet};

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

impl TryFrom <(&Query<'_>, PgRow)> for ResultRow {
    type Error = anyhow::Error;

    fn try_from(value: (&Query<'_>, PgRow)) -> Result<Self, Self::Error> {
        use sqlx::Row;

        let (query, row) = value;
        
        let document_id: i32 = row.try_get("document_id")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;
        let updated_at: DateTime<Utc> = row.try_get("updated_at")?;

        let document = query.document_ref;
        
        let mut locale = None;
        if document.has_localization() {
            // TODO: localization column name must be specified once
            let val: String = row.try_get("locale")?;
            locale = Some(val);
        }
        
        let mut published_at = None;
        if document.has_draft_and_publish() {
            let val: Option<DateTime<Utc>> = row.try_get("published_at")?;
            published_at = val
        }

        let mut body = HashMap::new();
        for column in query.columns.iter() {
            if let Some(column_name) = column.attribute_name.as_ref() {
                let value: String = row.try_get(column.name)?;
                body.insert(column_name.to_string(), value);
            }
        }
        
        Ok(ResultRow { document_id, created_at, updated_at, published_at, locale, body })
    }
}

impl PersistenceAdapter {
    pub fn new(database: &'static Database) -> Self {
        Self { database }
    }
}

impl Persistence for PersistenceAdapter {
    async fn select_all(&self, query: Query<'_>) -> Result<impl ResultSet, anyhow::Error> {
        let sql = &query.sql;
        let mut db_rows = sqlx::query(sql).fetch(self.database.database_pool());
        
        let mut rows = Vec::new();
        
        use futures::TryStreamExt;
        while let Some(row) = db_rows.try_next().await? {
            let result_row = ResultRow::try_from((&query, row))?;
            rows.push(result_row);
        }
        
        Ok(ResultSetImpl { rows })
    }
}
