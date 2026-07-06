use crate::domain::document::{DocumentInstance, lifecycle::PublicationState};
use luminair_common::persistence::TableNameProviderConstructor;
use luminair_common::{
    AttributeId, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType,
    OWNING_DOCUMENT_ID_FIELD_NAME, PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME,
    REVISION_FIELD_NAME, SNAPSHOT_ID_FIELD_NAME, STATUS_FIELD_NAME, TARGET_DOCUMENT_ID_FIELD_NAME,
    UPDATED_FIELD_NAME, VERSION_FIELD_NAME,
};
use sea_query::{Alias, DynIden, Expr, ExprTrait, PostgresQueryBuilder, Query};
use sea_query_sqlx::{SqlxBinder, SqlxValues};
use uuid::Uuid;

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

pub fn build_snapshot_insert(
    document: &DocumentType,
    instance: &DocumentInstance,
) -> (String, SqlxValues) {
    let table_name = format!("{}_snapshots", document.id.normalized());
    let table = sea_query::TableName::from(table_name);

    let mut columns: Vec<sea_query::DynIden> = vec![
        DOCUMENT_ID_FIELD_NAME.into(),
        PUBLISHED_FIELD_NAME.into(),
        PUBLISHED_BY_FIELD_NAME.into(),
        REVISION_FIELD_NAME.into(),
    ];

    for field in &document.fields {
        columns.push(field.id.normalized().into());
    }

    let mut values = vec![
        instance.document_id.0.into(),
        match &instance.content.publication_state {
            PublicationState::Published { published_at, .. } => Expr::from(*published_at),
            _ => Expr::null(),
        },
        match &instance.content.publication_state {
            PublicationState::Published { published_by, .. } => {
                if let Some(user_id) = published_by {
                    Expr::from(user_id.to_string())
                } else {
                    Expr::null()
                }
            }
            _ => Expr::null(),
        },
        match &instance.content.publication_state {
            PublicationState::Published { revision, .. } | PublicationState::Draft { revision } => {
                (*revision).into()
            }
        },
    ];

    for field in &document.fields {
        let expr = match instance.content.fields.get(&field.id) {
            Some(val) => val.into(),
            None => Expr::null(),
        };
        values.push(expr);
    }

    Query::insert()
        .into_table(table)
        .columns(columns)
        .values_panic(values)
        .returning(Query::returning().column(Alias::new(SNAPSHOT_ID_FIELD_NAME)))
        .build_sqlx(PostgresQueryBuilder)
}

pub fn build_copy_relations_to_snapshots(
    main_document: &DocumentType,
    relation_attr: &AttributeId,
    document_id: Uuid,
    snapshot_id: i64,
) -> (String, SqlxValues) {
    let working_table = main_document.relation_table(relation_attr);
    let snapshot_relation_table = main_document.relation_snapshot_table(relation_attr);

    let select_query = Query::select()
        .expr(Expr::val(snapshot_id))
        .column(Alias::new(TARGET_DOCUMENT_ID_FIELD_NAME))
        .column(Alias::new(OWNING_DOCUMENT_ID_FIELD_NAME))
        .from(working_table)
        .and_where(Expr::col(Alias::new(OWNING_DOCUMENT_ID_FIELD_NAME)).eq(document_id))
        .to_owned();

    let mut insert_query = Query::insert();
    insert_query
        .into_table(snapshot_relation_table)
        .columns(vec![
            Alias::new(SNAPSHOT_ID_FIELD_NAME),
            Alias::new(TARGET_DOCUMENT_ID_FIELD_NAME),
            Alias::new(OWNING_DOCUMENT_ID_FIELD_NAME),
        ]);
    insert_query
        .select_from(select_query)
        .expect("valid select_from query");

    insert_query.build_sqlx(PostgresQueryBuilder)
}
