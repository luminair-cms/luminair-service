use std::borrow::Cow;

use luminair_common::persistence::TableNameProvider;

use crate::domain::sql::SqlParameterRef;

pub struct CreateStatement<'a> {
    table: TableNameProvider<'a>,
    columns: Vec<Cow<'a, str>>,
    params: Vec<SqlParameterRef>,
    returning: Option<Cow<'a, str>>,
}

impl<'a> CreateStatement<'a> {
    pub fn new(
        table: TableNameProvider<'a>,
        columns: Vec<Cow<'a, str>>,
    ) -> Self {
        Self {
            table,
            columns,
            params: vec![],
            returning: None,
        }
    }
    
    pub fn with_params(mut self, params: Vec<SqlParameterRef>) -> Self {
        self.params = params;
        self
    }
    
    pub fn returning(mut self, column: Cow<'a, str>) -> Self {
        self.returning = Some(column);
        self
    }

    pub fn to_sql(&self) -> (String, Vec<SqlParameterRef>) {
        let mut sql = String::new();

        sql.push_str(&format!("INSERT INTO {} (", self.table.table_name()));
        sql.push_str(&self.columns.join(","));
        sql.push_str(") VALUES (");

        // Enumerate params as ${1},${2} etc
        let placeholders: Vec<String> =
            (1..=self.params.len()).map(|i| format!("${}", i)).collect();
        sql.push_str(&placeholders.join(","));

        sql.push_str(")");
        
        if let Some(ref column) = self.returning {
            sql.push_str(&format!(" RETURNING {}", column));
        }

        (sql, self.params.clone())
    }
}

#[cfg(test)]
mod tests {
    use luminair_common::{
        entities::DocumentKind,
        test_utils::{make_document_fields, make_document_type, make_uid_document_field},
    };

    use super::*;

    #[test]
    fn test_to_sql_with_multiple_columns_and_params() {
        let columns = vec![
            Cow::Borrowed("id"),
            Cow::Borrowed("name"),
            Cow::Borrowed("value"),
        ];
        let params = vec![
            SqlParameterRef::from(1),
            SqlParameterRef::from(2),
            SqlParameterRef::from(3),
        ];

        let document_fields = make_document_fields();
        let document = make_document_type(
            "test_table",
            DocumentKind::Collection,
            "test_table",
            "test_tables",
            document_fields,
        );

        let stmt = CreateStatement {
            table: TableNameProvider::MainTable { document },
            columns,
            params,
            returning: None,
        };

        let (sql, result_params) = stmt.to_sql();

        assert_eq!(
            sql,
            "INSERT INTO test_table (id,name,value) VALUES ($1,$2,$3)"
        );
        assert_eq!(result_params.len(), 3);
        assert_eq!(result_params[0].index(), 1);
        assert_eq!(result_params[1].index(), 2);
        assert_eq!(result_params[2].index(), 3);
    }

    #[test]
    fn test_to_sql_with_single_column() {
        let columns = vec![Cow::Borrowed("id")];
        let params = vec![SqlParameterRef::from(42)];

        let document_fields = vec![make_uid_document_field()];
        let document = make_document_type(
            "test_table",
            DocumentKind::Collection,
            "test_table",
            "test_tables",
            document_fields,
        );

        let stmt = CreateStatement {
            table: TableNameProvider::MainTable { document },
            columns,
            params,
            returning: None,
        };

        let (sql, result_params) = stmt.to_sql();

        assert_eq!(result_params.len(), 1);
        assert_eq!(result_params[0].index(), 42);
    }
}
