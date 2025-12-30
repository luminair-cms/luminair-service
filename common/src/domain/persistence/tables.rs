/// Represents table in a database, used for ddl generation
#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub foreign_keys: Vec<ForeignKeyConstraint>,
    pub indexes: Vec<Index>,
}

/// Represents one column in the database table
#[derive(Debug)]
pub struct Column {
    pub name: String,
    pub attribute_name: Option<String>,
    pub column_type: ColumnType,
    pub column_length: Option<usize>,
    pub not_null: bool,
    pub unique: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

/// Represents Column types
#[derive(Debug)]
pub enum ColumnType {
    Serial,
    Uuid,
    Text,
    Varchar,
    Integer,
    Decimal,
    Date,
    TimestampTZ,
    Boolean,
}

/// Represents foreign key constraint in the database table
#[derive(Debug)]
pub struct ForeignKeyConstraint {
    pub table_name: String,
    pub column_name: String,
    pub referenced_table_name: String,
    pub referenced_column_name: String,
}

/// Represents an index in the database table
#[derive(Debug)]
pub struct Index {
    pub table_name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

impl Table {
    pub fn new(
        name: String,
        columns: Vec<Column>,
        foreign_keys: Vec<ForeignKeyConstraint>,
        indexes: Vec<Index>,
    ) -> Self {
        Self {
            name,
            columns,
            foreign_keys,
            indexes,
        }
    }
}

impl Column {
    pub fn new<T: Into<String>>(
        name: T,
        attribute_name: Option<T>,
        column_type: ColumnType,
        column_length: Option<usize>,
        not_null: bool,
        unique: bool,
        default_value: Option<T>,
    ) -> Self {
        let primary_key = false;
        Self {
            name: name.into(),
            attribute_name: attribute_name.map(|a| a.into()),
            column_type: column_type,
            column_length,
            not_null,
            unique,
            primary_key,
            default_value: default_value.map(T::into),
        }
    }

    pub fn primary_key<T: Into<String>>(name: T, column_type: ColumnType, column_length: Option<usize>) -> Self {
        Self {
            name: name.into(),
            attribute_name: None,
            column_type: column_type,
            column_length,
            not_null: false,
            unique: false,
            primary_key: true,
            default_value: None,
        }
    }
}

impl ForeignKeyConstraint {
    pub fn new<T: Into<String>>(
        table_name: T,
        column_name: T,
        referenced_table_name: T,
        referenced_column_name: T,
    ) -> Self {
        Self {
            table_name: table_name.into(),
            column_name: column_name.into(),
            referenced_table_name: referenced_table_name.into(),
            referenced_column_name: referenced_column_name.into(),
        }
    }
}

impl Index {
    pub fn new<T: Into<String>>(table_name: T, columns: Vec<T>, unique: bool) -> Self {
        Self {
            table_name: table_name.into(),
            columns: columns.into_iter().map(T::into).collect(),
            unique,
        }
    }
}
