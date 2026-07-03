use sea_query::{DynIden, Expr, ExprTrait, PostgresQueryBuilder, Query};
use sea_query_sqlx::{SqlxBinder, SqlxValues};

use uuid::Uuid;
use luminair_common::{DocumentType, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, STATUS_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME, REVISION_FIELD_NAME, PUBLISHED_FIELD_NAME, PUBLISHED_BY_FIELD_NAME};
use luminair_common::persistence::TableNameProviderConstructor;

pub fn insert_document(document: &DocumentType, params: Vec<Expr>) -> (String, SqlxValues) {
    let table = document.main_table();

    Query::insert()
        .into_table(table)
        .columns(main_insert_columns(document))
        .values_panic(params)
        .build_sqlx(PostgresQueryBuilder)
}

/// UPDATE {table} SET col1 = $1, col2 = $2, ... WHERE document_id = $id
///
/// `column_values` is the full set of columns to write. Identity columns
/// (`document_id`, `created_at`) are not included by callers; everything else
/// — `updated_at`, `version`, publication state, and dynamic fields — is.
pub fn update_document(
    document: &DocumentType,
    document_id: Uuid,
    column_values: Vec<(DynIden, Expr)>,
) -> (String, SqlxValues) {
    let table = document.main_table();

    Query::update()
        .table(table)
        .values(column_values)
        .and_where(Expr::col(DOCUMENT_ID_FIELD_NAME).eq(document_id))
        .build_sqlx(PostgresQueryBuilder)
}

pub fn delete_document(document: &DocumentType, id: Uuid) -> (String, SqlxValues) {
    let table = document.main_table();
    let document_id_column = Expr::col(("m", DOCUMENT_ID_FIELD_NAME));

    Query::delete()
        .from_table(table)
        .and_where(document_id_column.eq(id))
        .build_sqlx(PostgresQueryBuilder)
}

fn main_insert_columns(document: &DocumentType) -> Vec<DynIden> {
    let mut columns: Vec<DynIden> = vec![
        DOCUMENT_ID_FIELD_NAME.into(),
        STATUS_FIELD_NAME.into(),
        CREATED_FIELD_NAME.into(),
        UPDATED_FIELD_NAME.into(),
        VERSION_FIELD_NAME.into(),
        REVISION_FIELD_NAME.into(),
        PUBLISHED_FIELD_NAME.into(),
        PUBLISHED_BY_FIELD_NAME.into(),
    ];

    for field in &document.fields {
        columns.push(field.id.normalized().into());
    }
    columns
}
