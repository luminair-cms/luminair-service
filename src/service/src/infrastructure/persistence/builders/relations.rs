use crate::domain::query::DocumentStatus;
use crate::infrastructure::persistence::builders::main_select_columns;
use luminair_common::persistence::TableNameProviderConstructor;
use luminair_common::{
    AttributeId, DOCUMENT_ID_FIELD_NAME, DocumentType, OWNING_DOCUMENT_ID_FIELD_NAME,
    SNAPSHOT_ID_FIELD_NAME, STATUS_FIELD_NAME, TARGET_DOCUMENT_ID_FIELD_NAME, VERSION_FIELD_NAME,
};
use sea_query::extension::postgres::PgExpr;
use sea_query::{
    Alias, ColumnRef, DynIden, Expr, ExprTrait, JoinType, Order, PostgresQueryBuilder, Query,
};
use sea_query_sqlx::{SqlxBinder, SqlxValues};
use uuid::Uuid;

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
    let related_table =
        if status == DocumentStatus::Published && related_document.has_draft_and_publish() {
            related_document.snapshot_table()
        } else {
            related_document.main_table()
        };

    let relation_table =
        if status == DocumentStatus::Published && main_document.has_draft_and_publish() {
            main_document.relation_snapshot_table(relation_attr)
        } else {
            main_document.relation_table(relation_attr)
        };

    let owning_document_id_column = ("r", OWNING_DOCUMENT_ID_FIELD_NAME);

    let mut columns = main_select_columns(related_document, status);
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

    let (status_expr, version_expr) =
        if status == DocumentStatus::Published && related_document.has_draft_and_publish() {
            (Expr::cust("'PUBLISHED'"), Expr::cust("0"))
        } else {
            (
                Expr::col(("m", STATUS_FIELD_NAME)),
                Expr::col(("m", VERSION_FIELD_NAME)),
            )
        };

    select.expr_as(status_expr, Alias::new("status"));
    select.expr_as(version_expr, Alias::new("version"));

    if let Some(condition) = crate::infrastructure::persistence::builders::find::build_condition(
        filter,
        related_document,
        "m",
    ) {
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
        .on_conflict(
            sea_query::OnConflict::columns(vec![
                Alias::new(OWNING_DOCUMENT_ID_FIELD_NAME),
                Alias::new(TARGET_DOCUMENT_ID_FIELD_NAME),
            ])
            .do_nothing()
            .to_owned(),
        )
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

/// SELECT target_document_id FROM {relation_snapshot_table} WHERE owning_document_id = $1
pub fn query_snapshot_relation_target_ids(
    main_document: &DocumentType,
    relation_attr: &AttributeId,
    document_id: Uuid,
) -> (String, SqlxValues) {
    let relation_snapshot_table = main_document.relation_snapshot_table(relation_attr);
    let target_id_col = TARGET_DOCUMENT_ID_FIELD_NAME;
    let owning_id_col = OWNING_DOCUMENT_ID_FIELD_NAME;

    Query::select()
        .column(Alias::new(target_id_col))
        .from(relation_snapshot_table)
        .and_where(Expr::col(Alias::new(owning_id_col)).eq(document_id))
        .build_sqlx(PostgresQueryBuilder)
}

/// SELECT target_document_id FROM {relation_table} WHERE owning_document_id = $1
pub fn query_working_relation_target_ids(
    main_document: &DocumentType,
    relation_attr: &AttributeId,
    document_id: Uuid,
) -> (String, SqlxValues) {
    let relation_table = main_document.relation_table(relation_attr);
    let target_id_col = TARGET_DOCUMENT_ID_FIELD_NAME;
    let owning_id_col = OWNING_DOCUMENT_ID_FIELD_NAME;

    Query::select()
        .column(Alias::new(target_id_col))
        .from(relation_table)
        .and_where(Expr::col(Alias::new(owning_id_col)).eq(document_id))
        .build_sqlx(PostgresQueryBuilder)
}

/// INSERT INTO {relation_snapshot_table} (snapshot_id, target_document_id, owning_document_id) VALUES ($1, $2, $3)
pub fn insert_relation_snapshot_entry(
    main_document: &DocumentType,
    relation_attr: &AttributeId,
    snapshot_id: i64,
    owning_document_id: Uuid,
    target_document_id: Uuid,
) -> (String, SqlxValues) {
    let relation_snapshot_table = main_document.relation_snapshot_table(relation_attr);
    let snapshot_id_col = SNAPSHOT_ID_FIELD_NAME;
    let target_id_col = TARGET_DOCUMENT_ID_FIELD_NAME;
    let owning_id_col = OWNING_DOCUMENT_ID_FIELD_NAME;

    Query::insert()
        .into_table(relation_snapshot_table)
        .columns(vec![
            Alias::new(snapshot_id_col),
            Alias::new(target_id_col),
            Alias::new(owning_id_col),
        ])
        .values_panic(vec![
            snapshot_id.into(),
            target_document_id.into(),
            owning_document_id.into(),
        ])
        .build_sqlx(PostgresQueryBuilder)
}

/// DELETE FROM {relation_snapshot_table} WHERE snapshot_id = $1 AND target_document_id = $2
pub fn delete_relation_snapshot_entry(
    main_document: &DocumentType,
    relation_attr: &AttributeId,
    snapshot_id: i64,
    target_document_id: Uuid,
) -> (String, SqlxValues) {
    let relation_snapshot_table = main_document.relation_snapshot_table(relation_attr);
    let snapshot_id_col = SNAPSHOT_ID_FIELD_NAME;
    let target_id_col = TARGET_DOCUMENT_ID_FIELD_NAME;

    Query::delete()
        .from_table(relation_snapshot_table)
        .and_where(Expr::col(Alias::new(snapshot_id_col)).eq(snapshot_id))
        .and_where(Expr::col(Alias::new(target_id_col)).eq(target_document_id))
        .build_sqlx(PostgresQueryBuilder)
}
