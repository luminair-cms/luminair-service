use luminair_common::domain::relations::Relation;
use luminair_common::domain::{
    attributes::AttributeType,
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

/// returns database tables for given documents schema, sorted conform dependency order
pub fn documents_into_tables(documents: &dyn Documents) -> Vec<Table> {
    let mut tables = Vec::new();
    let mut relation_tables = Vec::new();

    for d in documents.documents() {
        let document_tables = document_into_tables(d);
        tables.push(document_tables.main_table);
        if let Some(localization_table) = document_tables.localization_table {
            tables.push(localization_table);
        }
        relation_tables.extend(document_tables.relation_tables);
    }
    tables.extend(relation_tables);

    tables
}

struct DocumentTables {
    pub main_table: Table,
    pub localization_table: Option<Table>,
    pub relation_tables: Vec<Table>,
}

fn document_into_tables(document: &Document) -> DocumentTables {
    let table_name = document.id.normalized();
    let has_localization = document.has_localization();

    let (columns, localization_columns) = document_columns(document);

    let main_table = Table::new(table_name.clone(), columns, Vec::new(), Vec::new());

    let mut localization_table = None;
    if has_localization {
        localization_table = Some(gen_localization_table(&table_name, localization_columns));
    }

    let mut relation_tables = Vec::new();
    for relation in document.relations.iter() {
        if relation.relation_type.is_owning() {
            let relation_table = gen_relation_table(document, &table_name, relation);
            relation_tables.push(relation_table);
        }
    }

    DocumentTables { main_table, localization_table, relation_tables }
}

fn gen_relation_table(document: &Document, table_name: &String, relation: &Relation) -> Table {
    let target_document = relation.target_document();

    let relation_table_name = format!("{}_{}_relation", &table_name, relation.id.normalized());

    let owning_column_name = format!("{}_id", document.info.singular_name.normalized());
    let inverse_column_name = format!("{}_id", target_document.info.singular_name.normalized());

    let mut columns = vec![
        Column::primary_key("relation_id", "SERIAL"),
        Column::new(&owning_column_name as &str, "INTEGER", true, false, None),
        Column::new(&inverse_column_name as &str, "INTEGER", true, false, None),
    ];
    if relation.ordering {
        let ordering_column_name = format!("{}_order", inverse_column_name);
        columns.push(Column::new(&ordering_column_name as &str, "INTEGER", true, false, None));
    }

    let foreign_keys = vec![
        ForeignKeyConstraint::new(
            &relation_table_name as &str,
            &owning_column_name,
            table_name,
            "document_id",
        ),
        ForeignKeyConstraint::new(
            &relation_table_name as &str,
            &inverse_column_name,
            target_document.id.normalized().as_ref(),
            "document_id",
        ),
    ];

    let indexes = vec![
        Index::new(&relation_table_name, vec![&owning_column_name], false),
        Index::new(&relation_table_name, vec![&inverse_column_name], false),
    ];

    let table = Table::new(relation_table_name, columns, foreign_keys, indexes);
    table
}

fn gen_localization_table(table_name: &String, localization_columns: Vec<Column>) -> Table {
    let localization_table_name = format!("{}_localization", &table_name);
    let fkey_constraint = ForeignKeyConstraint::new(
        &localization_table_name as &str,
        "document_id",
        &table_name,
        "document_id",
    );
    let fkey_index = Index::new(&localization_table_name as &str, vec!["document_id"], false);

    Table::new(
        localization_table_name,
        localization_columns,
        vec![fkey_constraint],
        vec![fkey_index],
    )
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
        let column_type = match attribute.attribute_type {
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
            Column::new(&attribute.id.normalized() as &str, column_type, attribute.required, attribute.unique, None);

        if attribute.localized {
            localization_columns.push(column);
        } else {
            columns.push(column);
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
