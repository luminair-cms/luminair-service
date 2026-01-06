use std::{borrow::Cow, collections::HashMap};

use luminair_common::{
    CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, LOCALE_FIELD_NAME, PUBLISHED_FIELD_NAME,
    UPDATED_FIELD_NAME,
    domain::persisted::{PersistedDocument, PersistedField},
};

/// Represents Query to Database:
/// SELECT
///     m.document_id,
///     m.created_at, m.updated_at, m.published_at,
///     l.locale,
///     m.field_1,..., m.field_N,
///     l.field_1, ... , l.field_N
/// FROM main_table m
/// JOIN localization_table l ON m.document_id = l.document_id
/// WHERE m.document_id = ?1
/// ORDER BY m.document_id, l.locale
pub struct Query<'a> {
    /// Sql statement for this query
    pub sql: String,
    /// Document has localization
    pub has_localization: bool,
    /// Document has draft&publish facility
    pub has_draft_and_publish: bool,
    /// Mapping from document attributes to query fields
    pub fields: HashMap<String, &'a PersistedField>,
}

/// Common columns

const DOCUMENT_ID_COLUMN: Column<'static> = Column {
    alias: "m",
    name: DOCUMENT_ID_FIELD_NAME,
};
const CREATED_COLUMN: Column<'static> = Column {
    alias: "m",
    name: CREATED_FIELD_NAME,
};
const UPDATED_COLUMN: Column<'static> = Column {
    alias: "m",
    name: UPDATED_FIELD_NAME,
};
const PUBLISHED_COLUMN: Column<'static> = Column {
    alias: "m",
    name: PUBLISHED_FIELD_NAME,
};
const LOCALE_COLUMN: Column<'static> = Column {
    alias: "l",
    name: LOCALE_FIELD_NAME,
};

pub struct QueryBuilder<'a> {
    pub document: &'a PersistedDocument,
    pub has_localization: bool,
    pub has_draft_and_publish: bool,
    pub find_by_document_id: bool,
}

impl<'a> QueryBuilder<'a> {
    pub fn new(document: &'a PersistedDocument) -> QueryBuilder<'a> {
        let has_localization = document.document_ref.has_localization();
        let has_draft_and_publish = document.document_ref.has_draft_and_publish();

        Self {
            document,
            has_localization,
            has_draft_and_publish,
            find_by_document_id: false,
        }
    }

    pub fn find_by_id(mut self) -> Self {
        self.find_by_document_id = true;
        self
    }

    pub fn build(self) -> Query<'a> {
        let parts = QueryParts::from(&self);
        let fields = self
            .document
            .fields
            .iter()
            .map(|(attribute_id, field)| (attribute_id.to_string(), field))
            .collect();
        Query {
            sql: parts.sql(),
            has_localization: self.has_localization,
            has_draft_and_publish: self.has_draft_and_publish,
            fields: fields,
        }
    }
}

/// Represents parts of query statement
struct QueryParts<'a> {
    pub from: Table<'a>,
    pub select: Vec<ColumnRef<'a>>,
    pub joins: Vec<Table<'a>>,
    pub conditions: Vec<Condition<'a>>,
    pub order: Vec<ColumnRef<'a>>,
}

impl<'a> From<&QueryBuilder<'a>> for QueryParts<'a> {
    fn from(value: &QueryBuilder<'a>) -> Self {
        let details = &value.document.details;
        let from = Table {
            name: &details.main_table_name,
            alias: "m",
        };

        let joins = if value.has_localization {
            vec![Table {
                name: &details.localization_table_name,
                alias: "l",
            }]
        } else {
            Vec::new()
        };

        let mut select = vec![
            Cow::Borrowed(&DOCUMENT_ID_COLUMN),
            Cow::Borrowed(&CREATED_COLUMN),
            Cow::Borrowed(&UPDATED_COLUMN),
        ];

        if value.has_draft_and_publish {
            select.push(Cow::Borrowed(&PUBLISHED_COLUMN));
        }
        if value.has_localization {
            select.push(Cow::Borrowed(&LOCALE_COLUMN));
        }

        for field in value.document.fields.values() {
            let alias = if field.localized { "l" } else { "m" };
            select.push(Cow::Owned(Column {
                alias,
                name: &field.table_column_name,
            }));
        }

        let conditions = if value.find_by_document_id {
            vec![Condition {
                column: Cow::Borrowed(&DOCUMENT_ID_COLUMN),
            }]
        } else {
            Vec::new()
        };

        let mut order = vec![Cow::Borrowed(&DOCUMENT_ID_COLUMN)];
        if value.has_localization {
            order.push(Cow::Borrowed(&LOCALE_COLUMN));
        };

        Self {
            from,
            select,
            joins,
            conditions,
            order,
        }
    }
}

impl<'a> QueryParts<'a> {
    fn sql(self) -> String {
        let from_exp: String = String::from(&self.from);
        let columns: Vec<String> = self.select.iter().map(|c| c.as_ref().into()).collect();
        let joins: Vec<String> = self
            .joins
            .iter()
            .map(|j| {
                format!(
                    "JOIN {} AS {} ON m.document_id = {}.document_id",
                    &j.name, j.alias, j.alias
                )
            })
            .collect();

        let conditions = if self.conditions.is_empty() {
            "".to_string()
        } else {
            let conditions: Vec<String> = self
                .conditions
                .iter()
                .map(|c| format!("{} = $1", c))
                .collect();
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let order: Vec<String> = self.order.iter().map(|c| c.as_ref().into()).collect();

        format!(
            "SELECT {} FROM {} {}{} ORDER BY {}",
            columns.join(","),
            from_exp,
            joins.join("\n"),
            conditions,
            order.join(",")
        )
    }
}

/// Represents a table in database
struct Table<'a> {
    pub name: &'a str,
    pub alias: &'static str,
}

impl<'a> From<&Table<'a>> for String {
    fn from(value: &Table) -> Self {
        format!("{} AS {}", value.name, value.alias)
    }
}

type ColumnRef<'a> = Cow<'a, Column<'a>>;

// Represents one column in the database table
#[derive(Clone)]
struct Column<'a> {
    pub alias: &'static str,
    pub name: &'a str,
}

impl<'a> Into<String> for &Column<'a> {
    fn into(self) -> String {
        format!("{}.{}", self.alias, self.name)
    }
}

struct Condition<'a> {
    pub column: ColumnRef<'a>,
}

impl<'a> Into<String> for &Condition<'a> {
    fn into(self) -> String {
        let column = self.column.as_ref();
        column.into()
    }
}

use std::fmt::Display;

impl<'a> Display for Condition<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str: String = self.into();
        f.write_str(str.as_str())
    }
}
