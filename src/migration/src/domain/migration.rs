use luminair_common::DocumentTypesRegistry;

use crate::domain::DocumentTables;
use crate::domain::dependency::{DependencyError, resolve_table_order};
use crate::domain::tables::{Column, ColumnType, ForeignKeyConstraint, Index, Table};

pub trait MigrationStep {
    fn ctx(&self) -> &'static str;
    fn ddls(self) -> Vec<String>;
}

#[derive(Debug, Clone)]
pub enum MigrationStepItem {
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

#[derive(Debug, Clone)]
pub struct CreateTableStep {
    pub ddls: Vec<String>,
}

impl CreateTableStep {
    pub fn new(database_schema: &str, table: &Table) -> Self {
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

#[derive(Debug, Clone)]
pub struct DropTableStep {
    pub table_name: String,
    pub schema: String,
}

impl DropTableStep {
    pub fn new(database_schema: &str, table_name: &str) -> Self {
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

/// Pure domain logic: Generates a list of migration steps based on the needed and actual database schemas.
pub fn plan_migration(
    needed_schema: &[Table],
    actual_schema: &[Table],
    database_schema: &str,
) -> Result<Vec<MigrationStepItem>, DependencyError> {
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
    let drop_order = match resolve_table_order(actual_schema) {
        Ok(ordered) => {
            // Creation order: independent first, dependent last.
            // Drop order: dependent first, independent last (so we reverse the creation order).
            let mut reversed = ordered
                .into_iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>();
            reversed.reverse();
            reversed
        }
        Err(DependencyError::CircularDependency(cycle_tables)) => {
            eprintln!("Circular dependency in database tables: {:?}", cycle_tables);
            // Fallback: use unordered names of actual tables
            // TODO: analyze this case more, we shouldn't have cycle dependencies in tables
            // TODO: if cycle dependencies can exist, use DROP CASCADE option
            actual_schema.iter().map(|t| t.name.clone()).collect()
        }
    };

    let obsolete_tables: Vec<String> = drop_order
        .into_iter()
        .filter(|name| !needed_names.contains(name))
        .collect();

    for table_name in obsolete_tables {
        migration_steps.push(MigrationStepItem::Drop(DropTableStep::new(
            database_schema,
            &table_name,
        )));
    }

    // create missing tables in needed order
    let ordered = resolve_table_order(needed_schema)?;
    for table in ordered {
        if !actual_names.contains(&table.name) {
            migration_steps.push(MigrationStepItem::Create(CreateTableStep::new(
                database_schema,
                table,
            )));
        }
    }

    Ok(migration_steps)
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
pub fn documents_into_tables(documents: &dyn DocumentTypesRegistry) -> Vec<Table> {
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

    #[test]
    fn test_drop_table_ddl() {
        let ddl = drop_table_ddl("my_schema", "my_table");
        assert_eq!(
            ddl,
            "DROP TABLE IF EXISTS \"my_schema\".\"my_table\" CASCADE"
        );
    }

    #[test]
    fn test_create_table_ddl_basic() {
        let id_column = Column::primary_key("id", ColumnType::Uuid, None);
        let name_column = Column::new("name", ColumnType::Text, None, true, false, None);
        let status_column = Column::new(
            "status",
            ColumnType::Text,
            None,
            true,
            false,
            Some("'DRAFT'"),
        );
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
        let fk = ForeignKeyConstraint::new("child_table", "parent_id", "parent_table", "id");
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

    fn make_test_table(name: &str) -> Table {
        Table::new(name.to_string(), vec![], vec![], vec![])
    }

    #[test]
    fn test_plan_migration_no_changes() {
        let t1 = make_test_table("t1");
        let needed = vec![t1.clone()];
        let actual = vec![t1];

        let steps = plan_migration(&needed, &actual, "public").unwrap();
        assert!(steps.is_empty());
    }

    #[test]
    fn test_plan_migration_create_table() {
        let t1 = make_test_table("t1");
        let needed = vec![t1];
        let actual = vec![];

        let steps = plan_migration(&needed, &actual, "public").unwrap();
        assert_eq!(steps.len(), 1);
        assert!(matches!(steps[0], MigrationStepItem::Create(_)));
    }

    #[test]
    fn test_plan_migration_drop_obsolete_table() {
        let t1 = make_test_table("t1");
        let needed = vec![];
        let actual = vec![t1];

        let steps = plan_migration(&needed, &actual, "public").unwrap();
        assert_eq!(steps.len(), 1);
        assert!(matches!(steps[0], MigrationStepItem::Drop(_)));
    }

    #[test]
    fn test_plan_migration_mixed_ops() {
        let t1 = make_test_table("t1");
        let t2 = make_test_table("t2");
        let needed = vec![t1]; // We want t1
        let actual = vec![t2]; // Database currently has t2

        let steps = plan_migration(&needed, &actual, "public").unwrap();
        assert_eq!(steps.len(), 2);
        // Drops obsolete tables first, then creates needed ones
        assert!(matches!(steps[0], MigrationStepItem::Drop(_)));
        assert!(matches!(steps[1], MigrationStepItem::Create(_)));
    }
}
