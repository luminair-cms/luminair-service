use luminair_common::domain::Documents;
use crate::domain::DocumentTables;
use crate::domain::tables::{Column, ColumnType, ForeignKeyConstraint, Index, Table};
use crate::domain::persistence::Persistence;

#[derive(Clone)]
pub struct Migration<P: Persistence> {
    documents: &'static dyn Documents,
    persistence: P,
}

pub trait MigrationStep {
    fn ctx(&self) -> &'static str;
    fn ddls(self) -> Vec<String>;
}

struct CreateTableStep {
    ddls: Vec<String>,
}

impl CreateTableStep {
    fn new(database_schema: &str, table: &Table) -> Self {
        let ddls = create_table_ddl(database_schema, table);
        Self { ddls }
    }
}

impl MigrationStep for CreateTableStep {
    fn ctx(&self) -> &'static str {
        "CREATE TABLE"
    }

    fn ddls(self) -> Vec<String> {
        self.ddls
    }
}

impl<P: Persistence> Migration<P> {
    pub fn new(documents: &'static dyn Documents, persistence: P) -> Self {
        Self {
            documents,
            persistence,
        }
    }

    // working with SERIAL types: https://www.bytebase.com/reference/postgres/how-to/how-to-use-serial-postgres/
    /// migrate database schema conform documents configuration
    pub async fn migrate(&self) -> Result<(), anyhow::Error> {
        let needed_schema = documents_into_tables(self.documents);
        let actual_schema = self.persistence.load().await?;

        let mut migration_steps = Vec::new();
        for table in needed_schema {
            if !actual_schema.contains(&table.name) {
                migration_steps.push(CreateTableStep::new(
                    self.persistence.datbase_schema(),
                    &table,
                ));
            }
        }

        self.persistence.apply_migration_steps(migration_steps).await?;

        Ok(())
    }
}

fn create_table_ddl(schema: &str, table: &Table) -> Vec<String> {
    let mut columns = Vec::new();
    let mut pk_columns = Vec::new();

    for column in table.columns.iter() {
        columns.push(column_ddl(column));
        if column.primary_key {
            pk_columns.push(&column.name as &str);
        }
    }

    let columns_sql = columns.join(",\n    ");
    let pk_columns_sql = pk_columns.join(",");

    let table_ddl = format!(
        "CREATE TABLE \"{}\".\"{}\" (\n    {},\n    PRIMARY KEY({})\n)",
        schema, table.name, columns_sql, pk_columns_sql
    );

    let mut ddls = vec![table_ddl];

    for fk in table.foreign_keys.iter() {
        ddls.push(create_fk_ddl(schema, fk));
    }

    for index in table.indexes.iter() {
        ddls.push(create_index_ddl(schema, index));
    }

    ddls
}

fn column_ddl(column: &Column) -> String {
    let ct = match column.column_type {
        ColumnType::Serial => "SERIAL",
        ColumnType::Uuid => "UUID",
        ColumnType::Text => "TEXT",
        ColumnType::Varchar => "VARCHAR",
        ColumnType::Integer => "INTEGER",
        ColumnType::Decimal => "DECIMAL",
        ColumnType::Date => "DATE",
        ColumnType::TimestampTZ => "TIMESTAMPTZ",
        ColumnType::Boolean => "BOOLEAN"
    };
    let mut sql = format!("\"{}\" {}", column.name, ct);
    if let Some(length) = column.column_length {
        sql.push_str(&format!("({})", length));
    }
    if column.not_null {
        sql.push_str(" NOT NULL");
    }
    if let Some(default_value) = &column.default_value {
        sql.push_str(format!(" DEFAULT {}", default_value).as_str());
    }
    if column.unique {
        sql.push_str(" UNIQUE");
    }
    sql
}

fn create_fk_ddl(schema: &str, fk: &ForeignKeyConstraint) -> String {
    format!(
        "ALTER TABLE \"{}\".\"{}\" ADD CONSTRAINT \"{}_{}_fkey\" FOREIGN KEY (\"{}\") REFERENCES \"{}\".\"{}\" (\"{}\") ON DELETE CASCADE",
        schema,
        fk.table_name,
        fk.table_name,
        fk.column_name,
        fk.column_name,
        schema,
        fk.referenced_table_name,
        fk.referenced_column_name
    )
}

fn create_index_ddl(schema: &str, index: &Index) -> String {
    let columns_sql = index.columns.join(", ");
    format!(
        "CREATE {} INDEX \"{}_{}_idx\" ON \"{}\".\"{}\" ({})",
        if index.unique { "UNIQUE" } else { "" },
        index.table_name,
        index.columns.join("_"),
        schema,
        index.table_name,
        columns_sql
    )
}

// returns database persistence for given documents schema, sorted conform dependency order
fn documents_into_tables(documents: &dyn Documents) -> Vec<Table> {
    let mut tables = Vec::new();
    let mut relation_tables = Vec::new();

    for d in documents.persisted_documents() {
        let d = DocumentTables::from(d);
        tables.push(d.main_table);
        if let Some(localization_table) = d.localization_table {
            tables.push(localization_table);
        }
        relation_tables.extend(d.relation_tables);
    }
    tables.extend(relation_tables);

    tables
}