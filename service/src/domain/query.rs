use std::{borrow::Cow, collections::HashMap};

use luminair_common::{
    CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, LOCALE_FIELD_NAME, PUBLISHED_FIELD_NAME,
    UPDATED_FIELD_NAME,
    domain::{attributes::RelationType, persisted::{PersistedDocument, PersistedField}},
};

/// Represents Query to Database:
/// query to main document:
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
/// query for populate:
/// SELECT
///     r.owning_column_name,
///     m.document_id,
//      m.created_at, m.updated_at, m.published_at,
///     l.locale,
///     m.field_1,..., m.field_N,
///     l.field_1, ... , l.field_N
/// FROM relation_table r
/// JOIN populated_table m ON l.document_id = r.populated_column_name
/// JOIN localization_table l ON m.document_id = l.document_id
/// WHERE r.main_column_name = ?1
/// ORDER BY m.document_id, l.locale
/// if relation.is_owning then:
///     main_column_name = owning_column_name, populated_column_name = inverse_column_name
/// else:
///     main_column_name = inverse_column_name, populated_column_name = owning_column_name
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

pub struct MainQueryBuilder<'a> {
    pub document: &'a PersistedDocument,
    pub find_by_document_id: bool
}

impl<'a> MainQueryBuilder<'a> {
    pub fn new(document: &'a PersistedDocument) -> MainQueryBuilder<'a> {
        Self {
            document,
            find_by_document_id: false,
        }
    }

    pub fn find_by_id(mut self) -> Self {
        self.find_by_document_id = true;
        self
    }

    pub fn build(self) -> Query<'a> {
        let builder = QueryBuilder::from(&self);
        let fields = self
            .document
            .fields
            .iter()
            .map(|(attribute_id, field)| (attribute_id.to_string(), field))
            .collect();
        Query {
            sql: builder.sql(),
            has_localization: self.document.has_localization,
            has_draft_and_publish: self.document.has_draft_and_publish,
            fields: fields,
        }
    }
}

pub struct PopulateQueryBuilder<'a> {
    pub relation_type: RelationType,
    pub target_document: &'a PersistedDocument,
    pub relation_table_name: String,
}

/// Represents parts of query statement
struct QueryBuilder<'a> {
    pub from: Table<'a>,
    pub select: Vec<ColumnRef<'a>>,
    pub joins: Vec<Table<'a>>,
    pub conditions: Vec<Condition<'a>>,
    pub order: Vec<ColumnRef<'a>>,
}

impl<'a> From<&MainQueryBuilder<'a>> for QueryBuilder<'a> {
    fn from(value: &MainQueryBuilder<'a>) -> Self {
        let details = &value.document.details;
        let from = Table {
            name: &details.main_table_name,
            alias: "m",
        };

        let joins = if value.document.has_localization {
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

        if value.document.has_draft_and_publish {
            select.push(Cow::Borrowed(&PUBLISHED_COLUMN));
        }
        if value.document.has_localization {
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

        let mut order = if value.find_by_document_id {
            Vec::new()
        } else {
            vec![Cow::Borrowed(&DOCUMENT_ID_COLUMN)]
        };
        if value.document.has_localization {
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

impl<'a> QueryBuilder<'a> {
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

        let where_clause = if self.conditions.is_empty() {
            "".to_string()
        } else {
            let conditions: Vec<String> = self
                .conditions
                .iter()
                .map(|c| format!("{} = $1", c))
                .collect();
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let order_by_clause = if self.order.is_empty() {
            "".to_owned()
        } else {
            let order_columns: Vec<String> = self.order.iter().map(|c| c.as_ref().into()).collect();
            format!(" ORDER BY {}", order_columns.join(","))
        };

        format!(
            "SELECT {} FROM {} {}{}{}",
            columns.join(","),
            from_exp,
            joins.join("\n"),
            where_clause,
            order_by_clause
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
