use sea_query::{ColumnRef, DynIden, Expr, ExprTrait, JoinType, Order, PostgresQueryBuilder, Query};
use sea_query::extension::postgres::PgExpr;
use sea_query_sqlx::{SqlxBinder, SqlxValues};
use uuid::Uuid;
use luminair_common::{AttributeId, DOCUMENT_ID_FIELD_NAME, DocumentType, OWNING_DOCUMENT_ID_FIELD_NAME, TARGET_DOCUMENT_ID_FIELD_NAME};
use luminair_common::persistence::{TableNameProvider, TableNameProviderConstructor};
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
    filter: &crate::domain::query::FilterExpression,
    status: DocumentStatus,
    params: Vec<Uuid>,
) -> (String, SqlxValues) {
    let (related_table, relation_table) = if status == DocumentStatus::Published {
        (related_document.snapshot_table(), main_document.relation_snapshot_table(relation_attr))
    } else {
        (related_document.main_table(), main_document.relation_table(relation_attr))
    };

    let owning_document_id_column = ("r", OWNING_DOCUMENT_ID_FIELD_NAME);

    let mut columns = main_select_columns(related_document);
    columns.push(owning_document_id_column.into());

    let mut select = Query::select();
    select
        .columns(columns)
        .from(relation_table)
        .join(
            JoinType::LeftJoin,
            related_table,
            ColumnRef::from(("m", DOCUMENT_ID_FIELD_NAME))
                .equals(ColumnRef::from(("r", TARGET_DOCUMENT_ID_FIELD_NAME))),
        )
        .and_where(Expr::col(owning_document_id_column).eq_any(params))
        .order_by(owning_document_id_column, Order::Asc);

    if let Some(condition) = crate::infrastructure::persistence::builders::find::build_condition(filter, related_document, "m") {
        select.cond_where(condition);
    }

    select.build_sqlx(PostgresQueryBuilder)
}

/// INSERT INTO {relation_table} (owning_document_id, target_document_id) VALUES ($1, $2)
pub fn insert_relation_entry(
    document: &DocumentType,
    relation_attr: &AttributeId,
    owning_document_id: Uuid,
    target_document_id: Uuid,
) -> (String, SqlxValues) {
    let relation_table = document.relation_table(relation_attr);

    let columns: Vec<DynIden> = vec![
        OWNING_DOCUMENT_ID_FIELD_NAME.into(),
        TARGET_DOCUMENT_ID_FIELD_NAME.into(),
    ];

    Query::insert()
        .into_table(relation_table)
        .columns(columns)
        .values_panic(vec![owning_document_id.into(), target_document_id.into()])
        .build_sqlx(PostgresQueryBuilder)
}

/// DELETE FROM {relation_table} WHERE owning_document_id = $1 AND target_document_id = $2
pub fn delete_relation_entry(
    document: &DocumentType,
    relation_attr: &AttributeId,
    owning_document_id: Uuid,
    target_document_id: Uuid,
) -> (String, SqlxValues) {
    let relation_table = document.relation_table(relation_attr);
    let owning_id_column = Expr::col(("r", OWNING_DOCUMENT_ID_FIELD_NAME));
    let target_id_column = Expr::col(("r", TARGET_DOCUMENT_ID_FIELD_NAME));

    Query::delete()
        .from_table(relation_table)
        .and_where(owning_id_column.eq(owning_document_id))
        .and_where(target_id_column.eq(target_document_id))
        .build_sqlx(PostgresQueryBuilder)
}
