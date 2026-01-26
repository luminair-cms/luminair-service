use std::borrow::Cow;

use luminair_common::{
    CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, PUBLISHED_FIELD_NAME,
    UPDATED_FIELD_NAME
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
    pub document: &'a Document,
    /// Sql statement for this query
    pub sql: String,
    pub  columns_indexes: ColumnsIndexes
}

impl <'a> From<QueryBuilder<'a>> for Query<'a> {
    fn from(value: QueryBuilder<'a>) -> Self {
        let sql = value.sql();
        Self {
            document: value.document,
            sql,
            columns_indexes: value.select.column_indexes()
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
    pub document: &'a Document,
    from: Table<'a>,
    select: Select<'a>,
    joins: Vec<Join<'a>>,
    conditions: Vec<Condition<'a>>,
    order: Vec<ColumnRef<'a>>,
    // TODO: add group by
    // TODO: add offset and limit
}

impl<'a> From<&'a Document> for QueryBuilder<'a> {
    fn from(value: &'a Document) -> Self {
        let from = Table {
            name: &value.persistence.main_table_name,
            alias: "m",
        };
        Self::new(value, from)
    }
}

impl<'a> QueryBuilder<'a> {
    fn new(document: &'a Document, from: Table<'a>) -> Self {
        let mut select = Select::new(document.has_draft_and_publish());

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
    
    pub fn from_relation(populated_document: &'a Document, relation: &'a DocumentRelation, related_document: &'a Document) -> QueryBuilder<'a> {
        let from = Table { name: &relation.relation_table_name as &str, alias: "r" };
        let mut builder = Self::new(related_document, from);
        
        let owning_column_name = &populated_document.persistence.relation_column_name as &str;
        
        builder.select.insert_owning_id(owning_column_name);

        let main_table = Table {
            name: &related_document.persistence.main_table_name,
            alias: "m",
        };
        
        let join = Join {
            join_table: main_table,
            main_column: Cow::Owned(Column {
                alias: "r",
                name: &related_document.persistence.relation_column_name
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
        let columns: Vec<String> = self.select.columns.iter().map(|c| c.as_ref().into()).collect();
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

struct Select<'a> {
    pub columns: Vec<ColumnRef<'a>>,
    has_draft_and_publish: bool,
    has_owning_column: bool,
}

pub struct ColumnsIndexes {
    has_draft_and_publish: bool,
    has_owning_column: bool,
}

impl<'a> Select<'a> {
    // TODO: for select add indexes of standard columns for select by ID
    // TODO: add index for optional PUBLISHED_COLUMN
    // TODO: add index for optional OWNING_COLUMN

    fn new(has_draft_and_publish: bool) -> Self {
        let mut columns = vec![
            Cow::Borrowed(&DOCUMENT_ID_COLUMN),
            Cow::Borrowed(&CREATED_COLUMN),
            Cow::Borrowed(&UPDATED_COLUMN),
        ];
        if has_draft_and_publish {
            columns.push(Cow::Borrowed(&PUBLISHED_COLUMN));
        }
        Self { columns, has_draft_and_publish, has_owning_column: false }
    }

    fn insert_owning_id(&mut self, owning_column_name: &'a str) {
        let column = Cow::Owned(Column {
            alias: "f",
            name: owning_column_name,
        });
        self.columns.insert(0, column);
        self.has_owning_column = true;
    }

    fn push(&mut self, column: ColumnRef<'a>) {
        self.columns.push(column);
    }

    fn column_indexes(&self) -> ColumnsIndexes {
        ColumnsIndexes {
            has_draft_and_publish: self.has_draft_and_publish,
            has_owning_column: self.has_owning_column,
        }
    }
}

impl ColumnsIndexes {
    pub fn owning_index(&self) -> Option<usize> {
        if self.has_owning_column { Some(0) } else { None}
    }

    pub fn document_id_index(&self) -> usize { if self.has_owning_column { 1 } else { 0 } }
    pub fn created_index(&self) -> usize { self.document_id_index() + 1 }
    pub fn updated_index(&self) -> usize { self.document_id_index() + 2 }

    pub fn published_index(&self) -> Option<usize> {
        if self.has_draft_and_publish { Some(self.document_id_index() + 3) } else { None }
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
use luminair_common::domain::attributes::DocumentRelation;
use luminair_common::domain::documents::Document;

impl<'a> Display for Condition<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str: String = self.into();
        f.write_str(str.as_str())
    }
}
