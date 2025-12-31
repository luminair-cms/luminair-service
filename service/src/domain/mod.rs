use std::collections::HashMap;
use chrono::{DateTime, Utc};
use luminair_common::{CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, LOCALE_FIELD_NAME, PUBLISHED_FIELD_NAME, UPDATED_FIELD_NAME};
use luminair_common::domain::persisted::PersistedDocument;
use luminair_common::domain::Documents;
use luminair_common::domain::documents::Document;

/// This trait used only for testing purposes.
pub trait HelloService: Send + Sync + 'static {
    fn hello(&self) -> impl Future<Output = Result<String, anyhow::Error>> + Send;
}

/// Service that translate requests to document model into requests to db
/// and provide serialize/deserialize
pub trait Persistence: Clone + Send + Sync + 'static {
    /// select all rows from database
    fn select_all(
        &self,
        query: Query<'_>,
    ) -> impl Future<Output = Result<impl ResultSet, anyhow::Error>> + Send;
}

pub trait ResultSet {
    fn into_rows(self) -> Vec<ResultRow>;
}

pub struct ResultRow {
    pub document_id: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub locale: Option<String>,
    pub body: HashMap<String,String>
}

//// The global application state shared between all request handlers.
pub trait AppState: Clone + Send + Sync + 'static {
    type H: HelloService;
    type P: Persistence;
    fn hello_service(&self) -> &Self::H;
    fn documents(&self) -> &'static dyn Documents;
    fn persistence(&self) -> &Self::P;
}

pub struct Query<'a> {
    pub sql: String,
    pub columns: Vec<Column<'a>>,
    pub document_ref: &'static Document
}

/// Represents Query to Database
/// select_all generates this select (filter and populate TBD)
/// SELECT
///     m.document_id,
///     m.created_at, m.updated_at, m.published_at,
///     l.locale,
///     m.field_1,..., m.field_N,
///     l.field_1, ... , l.field_N
/// FROM main_table m
/// JOIN localization_table l ON m.document_id = l.document_id
/// select_one adds to this Query expression WHERE m.document_id = ?
pub struct QueryBuilder<'a> {
    pub from: Table<'a>,
    pub joins: Vec<Table<'a>>,
    pub select: Vec<Column<'a>>,
    pub document_ref: &'static Document
}

impl <'a> QueryBuilder<'a> {
    pub fn select_all(document: &'a PersistedDocument) -> QueryBuilder<'a> {
        let details = &document.details;
        let from = Table {
            name: &details.main_table_name,
            alias: "m",
        };

        let has_localization = document.document_ref.has_localization();
        
        let joins = if has_localization {
            vec![Table {
                name: &details.localization_table_name,
                alias: "l",
            }]
        } else {
            Vec::new()
        };
        
        let mut select = document.fields.iter()
            .map(|(attribute_id, field)| {
                let alias = if field.localized { "l" } else { "m" };
                Column {
                    alias,
                    name: &field.table_column_name,
                    attribute_name: Some(attribute_id.as_ref()) 
                }
            })
            .collect::<Vec<_>>();
        
        // add special fields
        select.push(Column { alias: "m", name: DOCUMENT_ID_FIELD_NAME, attribute_name: None });
        select.push(Column { alias: "m", name: CREATED_FIELD_NAME, attribute_name: None });
        select.push(Column { alias: "m", name: UPDATED_FIELD_NAME, attribute_name: None });
        
        if document.document_ref.has_draft_and_publish() {
            select.push(Column { alias: "m", name: PUBLISHED_FIELD_NAME, attribute_name: None });
        }
        if has_localization {
            select.push(Column { alias: "l", name: LOCALE_FIELD_NAME, attribute_name: None });
        }

        QueryBuilder {
            from,
            joins,
            select,
            document_ref: document.document_ref
        }
    }

    pub fn generate(self) -> Query<'a> {
        let from_exp: String = String::from(&self.from);
        let columns: Vec<String> = self
            .select
            .iter()
            .map(|c| format!("{}.{}", c.alias, &c.name))
            .collect();
        let joins: Vec<String> = self
            .joins
            .iter()
            .map(|j| format!("JOIN {} AS {} ON m.document_id = {}.document_id", &j.name, j.alias, j.alias))
            .collect();

        let sql = format!(
            "SELECT {} FROM {}\n{}",
            columns.join(","),
            from_exp,
            joins.join("\n")
        );

        Query { 
            sql, 
            columns: self.select.into_iter().filter(|c| c.attribute_name.is_some()).collect(), 
            document_ref: self.document_ref
        }
    }
}

/// Represents table in a database, used for dml generation
pub struct Table<'a> {
    pub name: &'a str,
    pub alias: &'static str,
}

impl <'a> From<&Table<'a>> for String {
    fn from(value: &Table) -> Self {
        format!("{} AS {}", value.name, value.alias)
    }
}

// Represents one column in the database table
pub struct Column<'a> {
    pub alias: &'static str,
    pub name: &'a str,
    pub attribute_name: Option<&'a str>
}
