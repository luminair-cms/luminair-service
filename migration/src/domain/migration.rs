use luminair_common::{domain::documents::Documents, infrastructure::database::Database};

use crate::domain::tables::{Column, ForeignKeyConstraint, Index, Table, Tables, documents_into_tables};

pub trait Migration: Send + Sync + Clone + 'static {
    type D: Documents;
    type T: Tables;

    fn migrate(&self) -> impl Future<Output = Result<(), anyhow::Error>>;
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

pub async fn migration_steps(
    database_schema: &str,
    documents: &impl Documents,
    tables: &impl Tables,
) -> Result<Vec<impl MigrationStep>, anyhow::Error> {
    let needed_schema = documents_into_tables(documents);
    let actual_schema = tables.load().await?;

    let mut result = Vec::new();

    for table in needed_schema {
        if !actual_schema.contains(&table.name) {
            result.push(CreateTableStep::new(database_schema, &table));
        }
    }

    Ok(result)
}

pub async fn apply_migration_steps(
    steps: Vec<impl MigrationStep>,
    database: &Database,
) -> Result<(), anyhow::Error> {
    use futures::stream::{self, StreamExt};

    let mut stream = stream::iter(steps);
    while let Some(step) = stream.next().await {
        let ctx = step.ctx();
        let ddls = step.ddls();
        database.excute_in_transaction(ddls, ctx).await?;
    }

    Ok(())
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
    let mut sql = format!("\"{}\" {}", column.name, column.column_type);
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
