use std::{borrow::Cow, collections::HashMap, fmt::{Display, Formatter}};
use std::fmt::write;
use anyhow::Error;
use luminair_common::{CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, PUBLISHED_FIELD_NAME, UPDATED_FIELD_NAME, database::Database};

/// Common columns

pub const DOCUMENT_ID_COLUMN: Column<'static> = Column {
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

pub struct DatabaseQuery<'a> {
    from: Table<'a>,
    select: Select<'a>,
    condition: Option<ConditionExpression<'a>>,
    pagination: Option<Pagination>
}

impl <'a> DatabaseQuery<'a> {
    pub fn new(from: Table<'a>, select: Select<'a>) -> Self {
        Self { from, select, condition: None, pagination: None }
    }
    
    pub fn condition(&mut self, column: ColumnRef<'a>, condition: Condition) {
        self.condition = Some(ConditionExpression { column, condition });
    }
    
    pub fn pagination(&mut self, offset: u16, next: u16) {
        self.pagination = Some(Pagination { offset, next })
    }
    
    pub async fn execute(self, database: &'static Database) -> Result<ResultSet, Error> {
        let sql = self.sql();
        let db_query =  sqlx::query(&sql);
        // TODO: bind
        let mut db_rows = db_query.fetch(database.database_pool());
        todo!()
    }

    fn sql(&self) -> String {
        let from_exp: String = String::from(&self.from);
        let columns: Vec<String> = self.select.columns.iter().map(|c| c.as_ref().into()).collect();

        let mut params_counter = 0;

        let where_clause = if let Some(ref condition) = self.condition {
            params_counter += 1;
            format!(" WHERE {} {}{}", condition.column, condition.condition, params_counter)
        } else {
            "".to_string()
        };

        let pagination_clause = if self.pagination.is_some() {
            params_counter += 1;
            format!(" OFFSET ${} ROWS FETCH NEXT ${} ROWS ONLY", params_counter, params_counter + 1)
        } else {
            "".to_string()
        };

        format!(
            "SELECT {} FROM {} {}{}",
            columns.join(","),
            from_exp,
            where_clause,
            pagination_clause
        )
    }
}

pub struct ResultSet {
    pub rows: Vec<ResultRow>
}

pub struct ResultRow {
    pub owning_id: Option<i32>,
    pub document_id: i32,
    pub fields: HashMap<String,FieldValue>,
}

pub enum FieldValue {
    Ordinal(String),
    Localized(HashMap<String,String>)
}

// TODO: Introduce typed values, such as i32, DateTime<Utc>, etc
// TODO: Introduce Optional values, such as published_at: Option<DateTime<Utc>>,

/// Represents a table in database
pub struct Table<'a> {
    pub name: &'a str,
    pub alias: &'static str,
}

impl<'a> From<&Table<'a>> for String {
    fn from(value: &Table) -> Self {
        format!("{} AS {}", value.name, value.alias)
    }
}

// TODO: must be columns with extractors (what can extract value from sqlx ResultSet)
// TODO: special columns DOCUMENT_ID, OWNING_ID
pub struct Select<'a> {
    pub columns: Vec<ColumnRef<'a>>,
}

impl<'a> Select<'a> {
    pub fn new(has_draft_and_publish: bool) -> Self {
        let mut columns = vec![
            Cow::Borrowed(&DOCUMENT_ID_COLUMN),
            Cow::Borrowed(&CREATED_COLUMN),
            Cow::Borrowed(&UPDATED_COLUMN),
        ];
        if has_draft_and_publish {
            columns.push(Cow::Borrowed(&PUBLISHED_COLUMN));
        }
        Self { columns }
    }
    
    pub fn insert(&mut self, column_name: &'a str, alias: &'static str) {
        let column = Cow::Owned(Column {
            alias,
            name: column_name,
        });
        self.columns.insert(0, column);
    }
    
    pub fn push(&mut self, column_name: &'a str, alias: &'static str) {
        let column = Cow::Owned(Column {
            alias,
            name: column_name,
        });
        self.columns.push(column);
    }
}

type ColumnRef<'a> = Cow<'a, Column<'a>>;

// Represents one column in the database table
#[derive(Clone)]
struct Column<'a> {
    pub alias: &'static str,
    pub name: &'a str,
}

impl<'a> Display for Column<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.alias, self.name)
    }
}

impl<'a> From<&Column<'a>> for String {
    fn from(value: &Column<'a>) -> Self {
        value.to_string()
    }
}

struct ConditionExpression<'a> {
    column: ColumnRef<'a>,
    condition: Condition,
}

pub enum Condition {
    Equals(i32),
    InCollection(Vec<i32>),
}

impl <'a> Display for Condition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Condition::Equals(_) => write!(f, " = "),
            Condition::InCollection(_) => write!(f, " IN")
        }
    }
}

struct Pagination {
    pub offset: u16,
    pub next: u16
}
