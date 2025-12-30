use std::collections::HashMap;
use chrono::{DateTime, Utc};
use luminair_common::domain::persistence::DocumentPersistence;
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
    pub fn select_all(document: &'a DocumentPersistence) -> QueryBuilder<'a> {
        let main_table = &document.main_table;

        let from = Table {
            name: &main_table.name,
            alias: "m",
        };

        let joins = if let Some(localization_table) = document.localization_table.as_ref() {
            vec![Table {
                name: &localization_table.name,
                alias: "l",
            }]
        } else {
            Vec::new()
        };
        
        let mut select = main_table.columns.iter()
            .map(|c| Column {
                alias: "m",
                name: &c.name,
                attribute_name:
                c.attribute_name.as_ref().map(|x| x.as_str()) })
            .collect::<Vec<_>>();

        if let Some(localization_table) = &document.localization_table {
            localization_table.columns.iter()
                .filter( |c| c.name != "document_id")
                .for_each(|c| select.push(Column {
                    alias: "l",
                    name: &c.name,
                    attribute_name:
                    c.attribute_name.as_ref().map(|x| x.as_str()) }));
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
