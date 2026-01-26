use std::collections::HashMap;
use chrono::{DateTime, Utc};
use luminair_common::infrastructure::database::Database;
use sqlx::{postgres::PgRow, types::Json};
use luminair_common::{CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, PUBLISHED_FIELD_NAME, UPDATED_FIELD_NAME};
use crate::domain::{FieldValue, Persistence, ResultRow, ResultSet, query::Query};

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
        let index_map = &query.columns_indexes;
        
        let owning_id = match index_map.owning_index() {
            Some(idx) => Some(row.try_get(idx)?),
            None => None
        };
        let document_id: i32 = row.try_get(index_map.document_id_index())?;
        let created_at: DateTime<Utc> = row.try_get(index_map.created_index())?;
        let updated_at: DateTime<Utc> = row.try_get(index_map.updated_index())?;
        
        let document = query.document;
        
        let published_at = match index_map.published_index() {
            Some(idx) => Some(row.try_get(idx)?),
            None => None
        };

        let mut fields = HashMap::new();
        for (attribute_id, field) in document.fields.iter() {
            let id = attribute_id.to_string();
            if field.localized {
                let value: Json<HashMap<String, String>> = row.try_get(field.table_column_name.as_str())?;
                fields.insert(id, FieldValue::Localized(value.0));
            } else {
                let value: String = row.try_get(field.table_column_name.as_str())?;
                fields.insert(id, FieldValue::Ordinal(value));
            }
        }
        
        Ok(ResultRow { owning_id, document_id, created_at, updated_at, published_at, fields })
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
