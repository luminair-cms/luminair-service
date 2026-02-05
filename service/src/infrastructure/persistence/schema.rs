use std::borrow::Cow;

use luminair_common::DocumentTypeId;

// Represents a table in database
pub struct Table<'a> {
    pub name: &'a str,
    pub alias: &'static str,
}

impl <'a> Table<'a> {
    /// Get qualified table name with alias
    pub fn qualified(&self) -> String {
        format!("\"{}\" AS \"{}\"", self.name, self.alias)
    }
}

impl From<&DocumentTypeId> for Table<'_> {
    fn from(value: &DocumentTypeId) -> Self {
        Table {
            name: value.normalized().as_str(),
            alias: "m",
        }
    }
}

/// Represents one column in the database table
#[derive(Clone)]
pub struct Column<'a> {
    pub qualifier: &'static str,
    pub name: &'a str,
}

impl <'a> Column<'a> {
    /// Get qualified column name
    pub fn qualified(&self) -> String {
        format!("\"{}\".\"{}\"", self.qualifier, self.name)
    }
}

/// Column reference which can be either borrowed or owned
pub type ColumnRef<'a> = Cow<'a, Column<'a>>;
