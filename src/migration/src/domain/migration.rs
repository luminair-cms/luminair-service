use luminair_common::DocumentTypesRegistry;

use crate::domain::DocumentTables;
use crate::domain::persistence::Persistence;
use crate::domain::tables::{Column, ColumnType, ForeignKeyConstraint, Index, IntegerSize, Table};

#[derive(Clone)]
pub struct Migration<P: Persistence> {
    documents: &'static dyn DocumentTypesRegistry,
    persistence: P,
}

pub trait MigrationStep {
    fn ctx(&self) -> &'static str;
    fn ddls(self) -> Vec<String>;
}

enum MigrationStepItem {
    Create(CreateTableStep),
    Drop(DropTableStep),
}

impl MigrationStep for MigrationStepItem {
    fn ctx(&self) -> &'static str {
        match self {
            MigrationStepItem::Create(step) => step.ctx(),
            MigrationStepItem::Drop(step) => step.ctx(),
        }
    }

    fn ddls(self) -> Vec<String> {
        match self {
            MigrationStepItem::Create(step) => step.ddls(),
            MigrationStepItem::Drop(step) => step.ddls(),
        }
    }
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

struct DropTableStep {
    table_name: String,
    schema: String,
}

impl DropTableStep {
    fn new(database_schema: &str, table_name: &str) -> Self {
        Self {
            table_name: table_name.to_string(),
            schema: database_schema.to_string(),
        }
    }
}

impl MigrationStep for DropTableStep {
    fn ctx(&self) -> &'static str {
        "DROP TABLE"
    }

    fn ddls(self) -> Vec<String> {
        vec![drop_table_ddl(&self.schema, &self.table_name)]
    }
}

impl<P: Persistence> Migration<P> {
    pub fn new(documents: &'static dyn DocumentTypesRegistry, persistence: P) -> Self {
        Self {
            documents,
            persistence,
        }
    }

    // working with SERIAL types: https://www.bytebase.com/reference/postgres/how-to/how-to-use-serial-postgres/
    /// migrate database schema conform documents configuration
    pub async fn migrate(&self) -> Result<(), anyhow::Error> {
        // sorted conform dependency order
        let needed_schema = documents_into_tables(self.documents);
        let actual_schema = self.persistence.load().await?;

        let needed_names: std::collections::HashSet<String> = needed_schema
            .iter()
            .map(|table| table.name.clone())
            .collect();

        let mut migration_steps = Vec::new();

        let mut obsolete_tables: Vec<String> = actual_schema
            .iter()
            .cloned()
            .filter(|name| !needed_names.contains(name))
            .collect();

        obsolete_tables.sort_by(|a, b| drop_name_order(a, b));
        for table_name in obsolete_tables {
            migration_steps.push(MigrationStepItem::Drop(DropTableStep::new(
                self.persistence.database_schema(),
                &table_name,
            )));
        }

        // create missing tables in needed order
        for table in needed_schema {
            if !actual_schema.contains(&table.name) {
                migration_steps.push(MigrationStepItem::Create(CreateTableStep::new(
                    self.persistence.database_schema(),
                    &table,
                )));
            }
        }

        self.persistence
            .apply_migration_steps(migration_steps)
            .await?;

        Ok(())
    }
}

fn drop_table_ddl(schema: &str, table_name: &str) -> String {
    format!(
        "DROP TABLE IF EXISTS \"{}\".\"{}\" CASCADE",
        schema, table_name
    )
}

fn drop_name_order(a: &str, b: &str) -> std::cmp::Ordering {
    let a_is_relation = a.ends_with("_relation");
    let b_is_relation = b.ends_with("_relation");

    let a_is_identity = a.ends_with("_documents");
    let b_is_identity = b.ends_with("_documents");

    match (a_is_relation, b_is_relation) {
        (true, false) => return std::cmp::Ordering::Less,
        (false, true) => return std::cmp::Ordering::Greater,
        _ => {}
    }

    match (a_is_identity, b_is_identity) {
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => a.cmp(b),
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
        ColumnType::Integer(size) => match size {
            IntegerSize::Int16 => "SMALLINT",
            IntegerSize::Int32 => "INT",
            IntegerSize::Int64 => "BIGINT",
        },
        ColumnType::Decimal { precision, scale } => &format!("DECIMAL({},{})", precision, scale),
        ColumnType::Date => "DATE",
        ColumnType::TimestampTZ => "TIMESTAMPTZ",
        ColumnType::Boolean => "BOOLEAN",
        ColumnType::JsonB => "JSONB",
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
    let mut ddl = format!(
        "CREATE {} INDEX \"{}_{}_idx\" ON \"{}\".\"{}\" ({})",
        if index.unique { "UNIQUE" } else { "" },
        index.table_name,
        index.columns.join("_"),
        schema,
        index.table_name,
        columns_sql
    );
    if let Some(where_clause) = &index.where_clause {
        ddl.push_str(&format!(" WHERE {}", where_clause));
    }
    ddl
}

// returns database persistence for given documents schema, sorted conform dependency order
fn documents_into_tables(documents: &dyn DocumentTypesRegistry) -> Vec<Table> {
    let mut tables = Vec::new();
    let mut identity_tables = Vec::new();
    let mut relation_tables = Vec::new();

    for d in documents.iterate() {
        let d = DocumentTables::new(d, documents);
        identity_tables.push(d.identity_table);
        tables.push(d.collection_table);
        relation_tables.extend(d.relation_tables);
    }

    let mut result = Vec::new();
    result.extend(identity_tables);
    result.extend(tables);
    result.extend(relation_tables);

    result
}
