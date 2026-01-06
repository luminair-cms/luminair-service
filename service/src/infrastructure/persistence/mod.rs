use std::collections::HashMap;
use chrono::{DateTime, Utc};
use luminair_common::infrastructure::database::Database;
use sqlx::postgres::PgRow;
use luminair_common::{CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, LOCALE_FIELD_NAME, PUBLISHED_FIELD_NAME, UPDATED_FIELD_NAME};
use crate::domain::{Persistence, ResultRow, ResultSet, query::Query};

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
        
        let document_id: i32 = row.try_get(DOCUMENT_ID_FIELD_NAME)?;
        let created_at: DateTime<Utc> = row.try_get(CREATED_FIELD_NAME)?;
        let updated_at: DateTime<Utc> = row.try_get(UPDATED_FIELD_NAME)?;
        
        let mut locale = None;
        if query.has_localization {
            let val: String = row.try_get(LOCALE_FIELD_NAME)?;
            locale = Some(val);
        }
        
        let mut published_at = None;
        if query.has_draft_and_publish {
            let val: Option<DateTime<Utc>> = row.try_get(PUBLISHED_FIELD_NAME)?;
            published_at = val
        }

        let mut fields = HashMap::new();
        let mut localized_fields = HashMap::new();
        for (attribute_id, field) in query.fields.iter() {
            let id = attribute_id.to_string();
            let value: String = row.try_get(field.table_column_name.as_str())?;
            if field.localized {
                localized_fields.insert(id, value);
            } else {
                fields.insert(id, value);
            }
        }
        
        Ok(ResultRow { document_id, created_at, updated_at, published_at, locale, fields, localized_fields })
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

    async fn select_by_id(&self, query: Query<'_>, id: i32) -> Result<impl ResultSet, anyhow::Error> {
        let sql = &query.sql;
        let mut db_rows = sqlx::query(sql).bind(id).fetch(self.database.database_pool());
        
        let mut rows = Vec::new();
        
        use futures::TryStreamExt;
        while let Some(row) = db_rows.try_next().await? {
            let result_row = ResultRow::try_from((&query, row))?;
            rows.push(result_row);
        }
        
        Ok(ResultSetImpl { rows })
    }
}
