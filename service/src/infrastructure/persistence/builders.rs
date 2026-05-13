use luminair_common::persistence::TableNameProvider;
use luminair_common::{
    CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType, ID_FIELD_NAME,
    INVERSE_ID_FIELD_NAME, OWNING_ID_FIELD_NAME, PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME,
    REVISION_FIELD_NAME, UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME,
};
use sea_query::extension::postgres::PgExpr;
use sea_query::{
    ColumnRef, DynIden, Expr, ExprTrait, Iden, InsertStatement, IntoColumnRef, JoinType, Order,
    PostgresQueryBuilder, Query, TableRef,
};
use sea_query_sqlx::{SqlxBinder, SqlxValues};
use std::convert::Into;
use uuid::Uuid;

pub fn query_find_document_by_id(
    document: &DocumentType,
    id: Uuid,
    query: &crate::domain::repository::query::DocumentInstanceQuery,
) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();
    let columns = main_select_columns(document);

    let document_id_column = Expr::col(("m", DOCUMENT_ID_FIELD_NAME));

    let mut select = Query::select();
    select.columns(columns).from(table).and_where(document_id_column.eq(id));

    if document.has_draft_and_publish() && !query.include_drafts {
        select.and_where(Expr::col(("m", PUBLISHED_FIELD_NAME)).is_not_null());
    }

    select.build_sqlx(PostgresQueryBuilder)
}

pub fn query_find_document_by_criteria(
    document: &DocumentType,
    query: &crate::domain::repository::query::DocumentInstanceQuery,
) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();
    let columns = main_select_columns(document);

    let mut select = Query::select();
    select.columns(columns).from(table);

    if document.has_draft_and_publish() && !query.include_drafts {
        select.and_where(Expr::col(("m", PUBLISHED_FIELD_NAME)).is_not_null());
    }

    select.build_sqlx(PostgresQueryBuilder)
}

/// SELECT r.owning_id, m.id, m.document_id, ...
/// FROM relation_table r
/// JOIN related_table m ON m.id = r.inverse_id
/// WHERE r.owning_id = ANY($1)
/// ORDER BY r.owning_id
pub fn query_find_related_documents(
    main_document: &DocumentType,
    related_document: &DocumentType,
    relation_attr: &luminair_common::AttributeId,
    params: Vec<i64>,
) -> (String, SqlxValues) {
    let related_table: TableNameProvider = related_document.into();
    let relation_table: TableNameProvider = (main_document, relation_attr).into();

    let owning_id_column = ("r", OWNING_ID_FIELD_NAME);

    let mut columns = main_select_columns(related_document);
    columns.push(owning_id_column.into());

    Query::select()
        .columns(columns)
        .from(relation_table)
        .join(
            JoinType::LeftJoin,
            related_table,
            ColumnRef::from(("r", OWNING_ID_FIELD_NAME))
                .equals(ColumnRef::from(("r", INVERSE_ID_FIELD_NAME))),
        )
        .and_where(Expr::col(owning_id_column).eq_any(params))
        .order_by(owning_id_column, Order::Asc)
        .build_sqlx(PostgresQueryBuilder)
}

pub fn insert_document(document: &DocumentType, params: Vec<Expr>) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();

    Query::insert()
        .into_table(table)
        .columns(main_insert_columns(document))
        .values_panic(params)
        .build_sqlx(PostgresQueryBuilder)
}

pub fn delete_document(document: &DocumentType, id: Uuid) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();
    let document_id_column = Expr::col(("m", DOCUMENT_ID_FIELD_NAME));
    
    Query::delete()
        .from_table(table)
        .and_where(document_id_column.eq(id))
        .build_sqlx(PostgresQueryBuilder)
}

const STANDARD_SELECT_COLUMNS: [(&str, &str); 7] = [
    ("m", ID_FIELD_NAME),
    ("m", DOCUMENT_ID_FIELD_NAME),
    ("m", CREATED_FIELD_NAME),
    ("m", UPDATED_FIELD_NAME),
    ("m", CREATED_BY_FIELD_NAME),
    ("m", UPDATED_BY_FIELD_NAME),
    ("m", VERSION_FIELD_NAME),
];

fn main_select_columns(document: &DocumentType) -> Vec<ColumnRef> {
    let mut columns: Vec<ColumnRef> = STANDARD_SELECT_COLUMNS.iter()
        .map(|c| (*c).into()).collect();

    if document.has_draft_and_publish() {
        columns.push(("m", PUBLISHED_FIELD_NAME).into());
        columns.push(("m", PUBLISHED_BY_FIELD_NAME).into());
        columns.push(("m", REVISION_FIELD_NAME).into());
    }

    for field in &document.fields {
        columns.push(("m", field.id.normalized()).into());
    }

    columns
}

fn main_insert_columns(document: &DocumentType) -> Vec<DynIden> {
    let mut columns: Vec<DynIden> = vec![
        DOCUMENT_ID_FIELD_NAME.into(),
        CREATED_FIELD_NAME.into(),
        UPDATED_FIELD_NAME.into(),
        // CREATED_BY_FIELD_NAME,
        // UPDATED_BY_FIELD_NAME,
        VERSION_FIELD_NAME.into(),
    ];

    if document.has_draft_and_publish() {
        columns.push(PUBLISHED_FIELD_NAME.into());
        // columns.push(PUBLISHED_BY_FIELD_NAME);
        columns.push(REVISION_FIELD_NAME.into());
    }

    for field in &document.fields {
        columns.push(field.id.normalized().into());
    }
    columns
}
