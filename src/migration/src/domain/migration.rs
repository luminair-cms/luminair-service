use luminair_common::DocumentTypesRegistry;

use crate::application::Persistence;
use crate::domain::DocumentTables;
use crate::domain::dependency::{DependencyError, resolve_table_order};
use crate::domain::tables::{Column, ColumnType, ForeignKeyConstraint, Index, Table};

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
    pub async fn migrate(&self, dry_run: bool) -> Result<(), anyhow::Error> {
        // sorted conform dependency order
        let needed_schema = documents_into_tables(self.documents);
        let actual_schema = self.persistence.load().await?;

        let needed_names: std::collections::HashSet<String> = needed_schema
            .iter()
            .map(|table| table.name.clone())
            .collect();

        let actual_names: std::collections::HashSet<String> = actual_schema
            .iter()
            .map(|table| table.name.clone())
            .collect();

        let mut migration_steps = Vec::new();

        // Resolve drop order of all actual tables from the database topologically
        let drop_order = match resolve_table_order(&actual_schema) {
            Ok(ordered) => {
                // Creation order: independent first, dependent last.
                // Drop order: dependent first, independent last (so we reverse the creation order).
                let mut reversed = ordered.into_iter().map(|t| t.name.clone()).collect::<Vec<_>>();
                reversed.reverse();
                reversed
            }
            Err(DependencyError::CircularDependency(cycle_tables)) => {
                eprintln!("Circular dependency in database tables: {:?}", cycle_tables);
                // Fallback: use unordered names of actual tables
                actual_schema.iter().map(|t| t.name.clone()).collect()
            }
        };

        let obsolete_tables: Vec<String> = drop_order
            .into_iter()
            .filter(|name| !needed_names.contains(name))
            .collect();

        for table_name in obsolete_tables {
            migration_steps.push(MigrationStepItem::Drop(DropTableStep::new(
                self.persistence.database_schema(),
                &table_name,
            )));
        }

        // create missing tables in needed order
        match resolve_table_order(&needed_schema) {
            Ok(ordered) => {
                for table in ordered {
                    if !actual_names.contains(&table.name) {
                        migration_steps.push(MigrationStepItem::Create(CreateTableStep::new(
                            self.persistence.database_schema(),
                            &table,
                        )));
                    }
                }
            }
            Err(DependencyError::CircularDependency(needed_schema)) => {
                eprintln!("Cannot resolve order, circular dependency: {:?}", needed_schema);
            }
        }

        if dry_run {
            println!("--- DRY-RUN: The following SQL DDL would be executed ---");
            if migration_steps.is_empty() {
                println!("No migration steps needed. Database schema is up to date.");
            } else {
                for step in migration_steps {
                    println!("-- Context: {}", step.ctx());
                    for ddl in step.ddls() {
                        println!("{};", ddl);
                    }
                }
            }
            return Ok(());
        }

        // TODO: add logs about count created/deleted tables
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
        ColumnType::Identity(size) => {
            let s = size.to_sql_type();
            &format!("{} GENERATED ALWAYS AS IDENTITY", s)
        }
        ColumnType::Uuid => "UUID",
        ColumnType::Text => "TEXT",
        ColumnType::Varchar => "VARCHAR",
        ColumnType::Integer(size) => size.to_sql_type(),
        ColumnType::Decimal { precision, scale } => 
            &format!("DECIMAL({},{})", precision, scale),
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

    for d in documents.iterate() {
        let doc_tables = DocumentTables::new(d, documents);
        tables.extend(doc_tables.tables);
    }

    tables
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tables::{Column, ColumnType, ForeignKeyConstraint, Index};
    use luminair_common::entities::IntegerSize;

    #[test]
    fn test_drop_table_ddl() {
        let ddl = drop_table_ddl("my_schema", "my_table");
        assert_eq!(ddl, "DROP TABLE IF EXISTS \"my_schema\".\"my_table\" CASCADE");
    }

    #[test]
    fn test_create_table_ddl_basic() {
        let id_column = Column::primary_key("id", ColumnType::Uuid, None);
        let name_column = Column::new("name", ColumnType::Text, None, true, false, None);
        let status_column = Column::new("status", ColumnType::Text, None, true, false, Some("'DRAFT'"));
        let columns = vec![id_column, name_column, status_column];
        let table = Table::new("my_table".to_string(), columns, vec![], vec![]);

        let ddls = create_table_ddl("my_schema", &table);
        assert_eq!(ddls.len(), 1);
        let ddl = &ddls[0];
        assert!(ddl.contains("CREATE TABLE \"my_schema\".\"my_table\""));
        assert!(ddl.contains("\"id\" UUID"));
        assert!(ddl.contains("\"name\" TEXT NOT NULL"));
        assert!(ddl.contains("\"status\" TEXT NOT NULL DEFAULT 'DRAFT'"));
        assert!(ddl.contains("PRIMARY KEY(id)"));
    }

    #[test]
    fn test_create_fk_ddl() {
        let fk = ForeignKeyConstraint::new(
            "child_table",
            "parent_id",
            "parent_table",
            "id",
        );
        let ddl = create_fk_ddl("my_schema", &fk);
        assert_eq!(
            ddl,
            "ALTER TABLE \"my_schema\".\"child_table\" ADD CONSTRAINT \"child_table_parent_id_fkey\" FOREIGN KEY (\"parent_id\") REFERENCES \"my_schema\".\"parent_table\" (\"id\") ON DELETE CASCADE"
        );
    }

    #[test]
    fn test_create_index_ddl() {
        let index = Index::new("my_table", vec!["col1", "col2"], false);
        let ddl = create_index_ddl("my_schema", &index);
        assert_eq!(
            ddl,
            "CREATE  INDEX \"my_table_col1_col2_idx\" ON \"my_schema\".\"my_table\" (col1, col2)"
        );

        let unique_index = Index::new("my_table", vec!["col1"], true);
        let ddl_unique = create_index_ddl("my_schema", &unique_index);
        assert_eq!(
            ddl_unique,
            "CREATE UNIQUE INDEX \"my_table_col1_idx\" ON \"my_schema\".\"my_table\" (col1)"
        );
    }
}
