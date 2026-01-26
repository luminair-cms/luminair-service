use std::borrow::Cow;

use luminair_common::{
    CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, PUBLISHED_FIELD_NAME,
    UPDATED_FIELD_NAME,
    domain::persisted::{PersistedDocument, PersistedRelation},
};

/// Represents Query to Database:
/// query to main document:
/// SELECT
///     m.document_id,
///     m.created_at, m.updated_at, m.published_at,
///     m.field_1,..., m.field_N,
///     l.field_1, ... , l.field_N
/// FROM main_table m
/// WHERE m.document_id = ?1
/// ORDER BY m.document_id
/// query for populate:
/// SELECT
///     r.owning_column_name,
///     m.document_id,
//      m.created_at, m.updated_at, m.published_at,
///     m.field_1,..., m.field_N,
///     l.field_1, ... , l.field_N
/// FROM relation_table r
/// JOIN populated_table m ON l.document_id = r.populated_column_name
/// WHERE r.main_column_name = ?1
/// ORDER BY m.document_id
/// if relation.is_owning then:
///     main_column_name = owning_column_name, populated_column_name = inverse_column_name
/// else:
///     main_column_name = inverse_column_name, populated_column_name = owning_column_name
pub struct Query<'a> {
    /// Query for Document
    pub document: &'a PersistedDocument,
    /// Sql statement for this query
    pub sql: String
}

impl <'a> From<QueryBuilder<'a>> for Query<'a> {
    fn from(value: QueryBuilder<'a>) -> Self {
        let sql = value.sql();
        Self {
            document: value.document,
            sql
        }
    }
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

/// Represents parts of query statement
pub struct QueryBuilder<'a> {
    pub document: &'a PersistedDocument,
    from: Table<'a>,
    select: Vec<ColumnRef<'a>>,
    joins: Vec<Join<'a>>,
    conditions: Vec<Condition<'a>>,
    order: Vec<ColumnRef<'a>>,
}

impl<'a> From<&'a PersistedDocument> for QueryBuilder<'a> {
    fn from(value: &'a PersistedDocument) -> Self {
        let from = Table {
            name: &value.details.main_table_name,
            alias: "m",
        };
        Self::new(value, from)
    }
}

impl<'a> QueryBuilder<'a> {
    fn new(document: &'a PersistedDocument, from: Table<'a>) -> Self {
        let mut select = vec![
            Cow::Borrowed(&DOCUMENT_ID_COLUMN),
            Cow::Borrowed(&CREATED_COLUMN),
            Cow::Borrowed(&UPDATED_COLUMN),
        ];

        if document.has_draft_and_publish {
            select.push(Cow::Borrowed(&PUBLISHED_COLUMN));
        }

        for field in document.fields.values() {
            select.push(Cow::Owned(Column {
                alias: "m",
                name: &field.table_column_name,
            }));
        }
        
        Self {
            document,
            from,
            select,
            joins: Vec::new(),
            conditions: Vec::new(),
            order: vec![Cow::Borrowed(&DOCUMENT_ID_COLUMN)]
        }
    }
    
    pub fn find_by_document_id(mut self) -> Query<'a> {
        self.conditions.push(Condition {
            column: Cow::Borrowed(&DOCUMENT_ID_COLUMN),
        });
        Query::from(self)
    }
    
    pub fn from_relation(populated_document: &'a PersistedDocument, relation: &'a PersistedRelation, related_document: &'a PersistedDocument) -> QueryBuilder<'a> {
        let from = Table { name: &relation.relation_table_name as &str, alias: "r" };
        let mut builder = Self::new(related_document, from);
        
        let owning_column_name = &populated_document.details.relation_column_name;
        
        builder.select.insert(0, Cow::Owned(Column {
            alias: "r",
            name: owning_column_name,
        }));

        let main_table = Table {
            name: &related_document.details.main_table_name,
            alias: "m",
        };
        
        let join = Join {
            join_table: main_table,
            main_column: Cow::Owned(Column {
                alias: "r",
                name: &related_document.details.relation_column_name
            }),
            join_column_name: Cow::Borrowed(DOCUMENT_ID_FIELD_NAME)
        };
        builder.joins.insert(0, join);
       
        builder
    }

    pub fn with_owning_id_condition(mut self, owning_column_name: &'a str) -> Self {
        self.conditions.push(Condition {
            column: Cow::Owned(Column {
                alias: "r",
                name: owning_column_name,
            }),
        });
        self
    }
    
    fn sql(&self) -> String {
        let from_exp: String = String::from(&self.from);
        let columns: Vec<String> = self.select.iter().map(|c| c.as_ref().into()).collect();
        let joins: Vec<String> = self
            .joins
            .iter()
            .map(|j| {
                format!(
                    "JOIN {} AS {} ON {}.{} = {}.{}",
                    &j.join_table.name, j.join_table.alias, 
                    j.main_column.alias, j.main_column.name,
                    j.join_table.alias, j.join_column_name
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

struct Join<'a> {
    pub join_table: Table<'a>,
    pub main_column: ColumnRef<'a>,
    pub join_column_name: Cow<'a, str>
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
