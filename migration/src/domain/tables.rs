use luminair_common::domain::{
    document_attributes::{AttributeBody, AttributeType},
    documents::{Document, Documents},
};

/// Represents table in a database, used for ddl generation
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub foreign_keys: Vec<ForeignKeyConstraint>,
    pub indexes: Vec<Index>,
}

/// Represents one column in the database table
pub struct Column {
    pub name: String,
    pub column_type: String,
    pub not_null: bool,
    pub unique: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

/// Represents foreign key constraint in the database table
pub struct ForeignKeyConstraint {
    pub table_name: String,
    pub column_name: String,
    pub referenced_table_name: String,
    pub referenced_column_name: String,
}

/// Represents an index in the database table
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
        column_type: T,
        not_null: bool,
        unique: bool,
        default_value: Option<T>,
    ) -> Self {
        let primary_key = false;
        Self {
            name: name.into(),
            column_type: column_type.into(),
            not_null,
            unique,
            primary_key,
            default_value: default_value.map(T::into),
        }
    }

    pub fn primary_key<T: Into<String>>(name: T, column_type: T) -> Self {
        Self {
            name: name.into(),
            column_type: column_type.into(),
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

pub  fn documents_into_tables(documents: &impl Documents) -> Vec<Table> {
    let mut tables = Vec::new();

    for d in documents.documents() {
        let mut document_tables = document_into_tables(d);
        tables.append(&mut document_tables);
    }

    tables
}

fn document_into_tables(document: &Document) -> Vec<Table> {
    let table_name = document.table_name();
    let has_localization = document.has_localization();

    let (columns, localization_columns) = document_columns(document);

    let main_table = Table::new(document.table_name(), columns, Vec::new(), Vec::new());

    let mut tables = vec![main_table];

    if has_localization {
        let localization_table_name = format!("{}_localization", &table_name);
        let fkey_constraint = ForeignKeyConstraint::new(
            &localization_table_name as &str,
            "document_id",
            &table_name,
            "document_id",
        );
        let fkey_index = Index::new(&localization_table_name as &str, vec!["document_id"], false);

        let localization_table = Table::new(
            localization_table_name,
            localization_columns,
            vec![fkey_constraint],
            vec![fkey_index],
        );

        tables.push(localization_table);
    }

    tables
}

fn document_columns(document: &Document) -> (Vec<Column>, Vec<Column>) {
    let mut columns = vec![Column::primary_key("document_id", "SERIAL")];
    let mut localization_columns = if document.has_localization() {
        vec![
            Column::primary_key("document_id", "INTEGER"),
            Column::primary_key("locale", "VARCHAR(2)"),
        ]
    } else {
        Vec::new()
    };

    for attribute in document.attributes.iter() {
        match &attribute.body {
            AttributeBody::Field {
                attribute_type,
                unique,
                required,
                localized,
                ..
            } => {
                let column_type = match attribute_type {
                    AttributeType::Uid => "TEXT",
                    AttributeType::Uuid => "UUID",
                    AttributeType::Text => "TEXT",
                    AttributeType::Integer => "INTEGER",
                    AttributeType::Decimal => "DECIMAL",
                    AttributeType::Date => "DATE",
                    AttributeType::DateTime => "TIMESTAMP",
                    AttributeType::Boolean => "BOOLEAN",
                };

                let column =
                    Column::new(attribute.id.as_ref(), column_type, *required, *unique, None);

                if *localized {
                    localization_columns.push(column);
                } else {
                    columns.push(column);
                }
            }
            _ => {},
        }
    }

    columns.push(Column::new(
        "created_at",
        "TIMESTAMP",
        true,
        false,
        Some("now()"),
    ));
    columns.push(Column::new("updated_at", "TIMESTAMP", false, false, None));
    if document.has_draft_and_publish() {
        columns.push(Column::new("published_at", "TIMESTAMP", false, false, None));
    }
    // TODO: add created_by_id, updated_by_id columns

    (columns, localization_columns)
}
