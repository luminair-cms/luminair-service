use chrono::{DateTime, Utc};
use luminair_common::domain::{
    attributes::AttributeBody,
    documents::{Document, Documents},
};

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
        query: &Query,
    ) -> impl Future<Output = Result<impl ResultSet, anyhow::Error>> + Send;
}

pub trait ResultSet {
    fn into_rows(self) -> Vec<ResultRow>;
}

pub struct ResultRow {
    pub document_id: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

//// The global application state shared between all request handlers.
pub trait AppState: Clone + Send + Sync + 'static {
    type H: HelloService;
    type P: Persistence;
    fn hello_service(&self) -> &Self::H;
    fn documents(&self) -> &'static dyn Documents;
    fn persistence(&self) -> &Self::P;
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
pub struct Query {
    pub from: Table,
    pub joins: Vec<Table>,
    pub select: Vec<Column>,
}

impl Query {
    pub fn select_all(document: &Document) -> Query {
        let main_table_name = document.id.normalized();
        let has_localization = document.has_localization();

        let from = Table {
            name: main_table_name,
            alias: "m",
        };

        let joins = if has_localization {
            let localization_table_name = format!("{}_localization", &from.name);
            vec![Table {
                name: localization_table_name,
                alias: "l",
            }]
        } else {
            Vec::new()
        };

        let mut select = vec![
            Column {
                alias: "m",
                name: "document_id".to_owned(),
            },
            Column {
                alias: "m",
                name: "created_at".to_owned(),
            },
            Column {
                alias: "m",
                name: "updated_at".to_owned(),
            },
        ];

        if document.has_draft_and_publish() {
            select.push(Column {
                alias: "m",
                name: "published_at".to_owned(),
            })
        }

        if has_localization {
            select.push(Column {
                alias: "l",
                name: "locale".to_owned(),
            });
        }

        let mut main_columns = Vec::new();
        let mut localization_columns = Vec::new();

        for attribute in document.attributes.iter() {
            let id = attribute.id.normalized();
            if let AttributeBody::Field { localized, .. } = &attribute.body {
                if *localized {
                    localization_columns.push(Column {
                        alias: "l",
                        name: id,
                    });
                } else {
                    main_columns.push(Column {
                        alias: "m",
                        name: id,
                    });
                }
            }
        }

        select.append(&mut main_columns);
        select.append(&mut localization_columns);

        Query {
            from,
            joins,
            select,
        }
    }

    pub fn generate_select(&self) -> String {
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

        format!(
            "SELECT {} FROM {}\n{}",
            columns.join(","),
            from_exp,
            joins.join("\n")
        )
    }
}

/// Represents table in a database, used for dml generation
pub struct Table {
    pub name: String,
    pub alias: &'static str,
}

impl From<&Table> for String {
    fn from(value: &Table) -> Self {
        format!("{} AS {}", value.name, value.alias)
    }
}

// Represents one column in the database table
pub struct Column {
    pub alias: &'static str,
    pub name: String,
}
