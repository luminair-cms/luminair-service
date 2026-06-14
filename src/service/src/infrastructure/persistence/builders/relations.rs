use sea_query::{ColumnRef, DynIden, Expr, ExprTrait, JoinType, Order, PostgresQueryBuilder, Query};
use sea_query::extension::postgres::PgExpr;
use sea_query_sqlx::{SqlxBinder, SqlxValues};
use uuid::Uuid;
use luminair_common::{AttributeId, DocumentType, DOCUMENT_ID_FIELD_NAME, ID_FIELD_NAME, INVERSE_ID_FIELD_NAME, OWNING_ID_FIELD_NAME};
use luminair_common::persistence::TableNameProvider;
use crate::domain::document::DatabaseRowId;
use crate::domain::query::DocumentStatus;
use crate::infrastructure::persistence::builders::main_select_columns;

/**
 * if query.status == DocumentStatus::Published:
 * 
 * SELECT r.owning_document_id, m.document_id, 'PUBLISHED' as status, ...
 * FROM article_categories_relation_snapshots r
 * JOIN related_table_snapshots m ON m.document_id = r.target_document_id
 * WHERE r.owning_document_id = ANY($1)
 * ORDER BY r.owning_document_id
 * 
 * if query.status == DocumentStatus::Draft:
 * 
 * SELECT r.owning_document_id, m.document_id, ...
 * FROM article_categories_relation r
 * JOIN related_table m ON m.document_id = r.target_document_id
 * WHERE r.owning_document_id = ANY($1)
 * ORDER BY r.owning_document_id
 * 
 */
pub fn query_find_related_documents(
    main_document: &DocumentType,
    related_document: &DocumentType,
    relation_attr: &AttributeId,
    status: DocumentStatus,
    params: Vec<Uuid>,
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
            ColumnRef::from(("m", ID_FIELD_NAME))
                .equals(ColumnRef::from(("r", INVERSE_ID_FIELD_NAME))),
        )
        .and_where(Expr::col(owning_id_column).eq_any(params))
        .order_by(owning_id_column, Order::Asc)
        .build_sqlx(PostgresQueryBuilder)
}

/// INSERT INTO {relation_table} (owning_id, inverse_id) VALUES ($1, $2)
pub fn insert_relation_entry(
    document: &DocumentType,
    relation_attr: &AttributeId,
    owning_id: DatabaseRowId,
    inverse_id: DatabaseRowId,
) -> (String, SqlxValues) {
    let relation_table: TableNameProvider = (document, relation_attr).into();

    let columns: Vec<DynIden> = vec![OWNING_ID_FIELD_NAME.into(), INVERSE_ID_FIELD_NAME.into()];

    Query::insert()
        .into_table(relation_table)
        .columns(columns)
        .values_panic(vec![owning_id.0.into(), inverse_id.0.into()])
        .build_sqlx(PostgresQueryBuilder)
}

/// DELETE FROM {relation_table} WHERE owning_id = $1 AND inverse_id = $2
pub fn delete_relation_entry(
    document: &DocumentType,
    relation_attr: &AttributeId,
    owning_id: DatabaseRowId,
    inverse_id: DatabaseRowId,
) -> (String, SqlxValues) {
    let relation_table: TableNameProvider = (document, relation_attr).into();
    let owning_id_column = Expr::col(("r", OWNING_ID_FIELD_NAME));
    let inverse_id_column = Expr::col(("r", INVERSE_ID_FIELD_NAME));

    Query::delete()
        .from_table(relation_table)
        .and_where(owning_id_column.eq(owning_id.0))
        .and_where(inverse_id_column.eq(inverse_id.0))
        .build_sqlx(PostgresQueryBuilder)
}

/// SELECT id FROM {table} WHERE document_id = $uuid
/// Used by `apply_relation_ops` to resolve a single UUID to its database row ID.
pub fn query_row_id_by_document_uuid(document: &DocumentType, uuid: Uuid) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();

    Query::select()
        .column(("m", ID_FIELD_NAME))
        .from(table)
        .and_where(Expr::col(("m", DOCUMENT_ID_FIELD_NAME)).eq(uuid))
        .build_sqlx(PostgresQueryBuilder)
}

/// SELECT id FROM {table} WHERE document_id = ANY($uuids)
/// Used by `apply_relation_ops` to batch-resolve related document UUIDs to row IDs.
pub fn query_row_ids_by_document_uuids(
    document: &DocumentType,
    uuids: Vec<Uuid>,
) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();

    Query::select()
        .column(("m", ID_FIELD_NAME))
        .from(table)
        .and_where(Expr::col(("m", DOCUMENT_ID_FIELD_NAME)).eq_any(uuids))
        .build_sqlx(PostgresQueryBuilder)
}
