use sea_query::{ColumnRef, Condition, Expr, ExprTrait, IntoIden, JoinType, Order, PostgresQueryBuilder, Query, SelectStatement, TableName, TableRef};
use sea_query_sqlx::{SqlxBinder, SqlxValues};
use uuid::Uuid;
use luminair_common::{DocumentType, DOCUMENT_ID_FIELD_NAME, ID_FIELD_NAME, CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME};
use luminair_common::persistence::TableNameProvider;
use crate::domain::query::{DocumentInstanceQuery, DocumentStatus, FilterExpression, SortDirection};
use crate::infrastructure::persistence::builders::main_select_columns;

fn snapshot_table(document: &DocumentType) -> TableRef {
    let table_name = format!("{}_snapshots", document.id.normalized());
    TableRef::Table(TableName::from(table_name), Some("s".into_iden()))
}

fn snapshot_select_columns(document: &DocumentType) -> Vec<ColumnRef> {
    let mut columns: Vec<ColumnRef> = vec![
        ("m", ID_FIELD_NAME).into(),
        ("m", DOCUMENT_ID_FIELD_NAME).into(),
        ("m", CREATED_FIELD_NAME).into(),
        ("m", UPDATED_FIELD_NAME).into(),
        ("m", CREATED_BY_FIELD_NAME).into(),
        ("m", UPDATED_BY_FIELD_NAME).into(),
        ("m", VERSION_FIELD_NAME).into(),
        ("s", PUBLISHED_FIELD_NAME).into(),
        ("s", PUBLISHED_BY_FIELD_NAME).into(),
        ("s", REVISION_FIELD_NAME).into(),
    ];

    for field in &document.fields {
        columns.push(("s", field.id.normalized()).into());
    }

    columns
}

fn build_revision_expression(document: &DocumentType) -> Expr {
    let snapshot_table_name = format!("{}_snapshots", document.id.normalized());
    Expr::cust(format!(
        "COALESCE((SELECT MAX({revision}) FROM {snapshot} WHERE document_id = m.document_id), 0) AS {revision}",
        revision = REVISION_FIELD_NAME,
        snapshot = snapshot_table_name,
    ))
}

fn build_draft_null_publication_expressions(document: &DocumentType, select: &mut Query) {
    if document.has_draft_and_publish() {
        select.column(Expr::cust(format!("NULL::timestamp with time zone AS {}", PUBLISHED_FIELD_NAME)));
        select.column(Expr::cust(format!("NULL::text AS {}", PUBLISHED_BY_FIELD_NAME)));
        select.column(build_revision_expression(document));
    }
}

pub fn query_find_document_by_id(
    document: &DocumentType,
    id: Uuid,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    if document.has_draft_and_publish() && query.status == DocumentStatus::Published {
        let snapshot_table_ref = snapshot_table(document);
        let main_table_ref: TableNameProvider = document.into();
        let columns = snapshot_select_columns(document);

        let mut select = Query::select();
        select
            .columns(columns)
            .from(snapshot_table_ref)
            .join(
                JoinType::LeftJoin,
                main_table_ref,
                ColumnRef::from(("m", DOCUMENT_ID_FIELD_NAME))
                    .equals(ColumnRef::from(("s", DOCUMENT_ID_FIELD_NAME))),
            )
            .and_where(Expr::col(("s", DOCUMENT_ID_FIELD_NAME)).eq(id))
            .and_where(Expr::cust(format!(
                "s.{} = (SELECT MAX({}) FROM {} WHERE document_id = s.document_id)",
                REVISION_FIELD_NAME,
                REVISION_FIELD_NAME,
                format!("{}_snapshots", document.id.normalized()),
            )));

        if let Some(condition) = build_condition(&query.filter, document, "s") {
            select.cond_where(condition);
        }

        select.build_sqlx(PostgresQueryBuilder)
    } else {
        let table: TableNameProvider = document.into();
        let mut select = Query::select();
        select.columns(main_select_columns(document)).from(table);

        if document.has_draft_and_publish() {
            build_draft_null_publication_expressions(document, &mut select);
        }

        select.and_where(Expr::col(("m", DOCUMENT_ID_FIELD_NAME)).eq(id));

        if let Some(condition) = build_condition(&query.filter, document, "m") {
            select.cond_where(condition);
        }

        select.build_sqlx(PostgresQueryBuilder)
    }
}

pub fn query_find_document_by_criteria(
    document: &DocumentType,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    if document.has_draft_and_publish() && query.status == DocumentStatus::Published {
        let snapshot_table_ref = snapshot_table(document);
        let main_table_ref: TableNameProvider = document.into();
        let columns = snapshot_select_columns(document);

        let mut select = Query::select();
        select
            .columns(columns)
            .from(snapshot_table_ref)
            .join(
                JoinType::LeftJoin,
                main_table_ref,
                ColumnRef::from(("m", DOCUMENT_ID_FIELD_NAME))
                    .equals(ColumnRef::from(("s", DOCUMENT_ID_FIELD_NAME))),
            )
            .and_where(Expr::cust(format!(
                "s.{} = (SELECT MAX({}) FROM {} WHERE document_id = s.document_id)",
                REVISION_FIELD_NAME,
                REVISION_FIELD_NAME,
                format!("{}_snapshots", document.id.normalized()),
            )));

        if let Some(condition) = build_condition(&query.filter, document, "s") {
            select.cond_where(condition);
        }

        for sort in &query.sort {
            let col = get_column_expr(&sort.field, document, "s");
            let order = match sort.direction {
                SortDirection::Ascending => Order::Asc,
                SortDirection::Descending => Order::Desc,
            };
            select.order_by_expr(col, order);
        }

        if let Some(limit) = query.limit {
            select.limit(limit as u64);
        }
        if let Some(offset) = query.offset {
            select.offset(offset as u64);
        }

        select.build_sqlx(PostgresQueryBuilder)
    } else {
        let table: TableNameProvider = document.into();
        let mut select = Query::select();
        select.columns(main_select_columns(document)).from(table);

        if document.has_draft_and_publish() {
            build_draft_null_publication_expressions(document, &mut select);
        }

        if let Some(condition) = build_condition(&query.filter, document, "m") {
            select.cond_where(condition);
        }

        for sort in &query.sort {
            let col = get_column_expr(&sort.field, document, "m");
            let order = match sort.direction {
                SortDirection::Ascending => Order::Asc,
                SortDirection::Descending => Order::Desc,
            };
            select.order_by_expr(col, order);
        }

        if let Some(limit) = query.limit {
            select.limit(limit as u64);
        }
        if let Some(offset) = query.offset {
            select.offset(offset as u64);
        }

        select.build_sqlx(PostgresQueryBuilder)
    }
}

pub fn query_count_documents(
    document: &DocumentType,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    if document.has_draft_and_publish() && query.status == DocumentStatus::Published {
        let snapshot_table_name = format!("{}_snapshots", document.id.normalized());
        let mut select = Query::select();
        select
            .column(Expr::cust("COUNT(DISTINCT s.document_id)"))
            .from(snapshot_table(document));
        select.and_where(Expr::cust(format!(
            "s.{} = (SELECT MAX({}) FROM {} WHERE document_id = s.document_id)",
            REVISION_FIELD_NAME,
            REVISION_FIELD_NAME,
            snapshot_table_name,
        )));

        if let Some(condition) = build_condition(&query.filter, document, "s") {
            select.cond_where(condition);
        }

        select.build_sqlx(PostgresQueryBuilder)
    } else {
        let table: TableNameProvider = document.into();
        let mut select = Query::select();
        select
            .column(Expr::col(("m", ID_FIELD_NAME)).count())
            .from(table);

        if let Some(condition) = build_condition(&query.filter, document, "m") {
            select.cond_where(condition);
        }

        select.build_sqlx(PostgresQueryBuilder)
    }
}

fn build_condition(filter: &FilterExpression, document: &DocumentType, alias: &str) -> Option<Condition> {
    match filter {
        FilterExpression::None => None,
        FilterExpression::And(left, right) => {
            let left_cond = build_condition(left, document, alias);
            let right_cond = build_condition(right, document, alias);
            match (left_cond, right_cond) {
                (Some(l), Some(r)) => Some(Condition::all().add(l).add(r)),
                (Some(l), None) => Some(l),
                (None, Some(r)) => Some(r),
                (None, None) => None,
            }
        }
        FilterExpression::Or(left, right) => {
            let left_cond = build_condition(left, document, alias);
            let right_cond = build_condition(right, document, alias);
            match (left_cond, right_cond) {
                (Some(l), Some(r)) => Some(Condition::any().add(l).add(r)),
                (Some(l), None) => Some(l),
                (None, Some(r)) => Some(r),
                (None, None) => None,
            }
        }
        _ => {
            if let Some(expr) = build_filter_expr(filter, document, alias) {
                Some(Condition::all().add(expr))
            } else {
                None
            }
        }
    }
}

fn build_filter_expr(filter: &FilterExpression, document: &DocumentType, alias: &str) -> Option<Expr> {
    match filter {
        FilterExpression::Equals { field, value } => Some(get_column_expr(field, document, alias).eq(Expr::from(value))),
        FilterExpression::NotEquals { field, value } => Some(get_column_expr(field, document, alias).ne(Expr::from(value))),
        FilterExpression::GreaterThan { field, value } => Some(get_column_expr(field, document, alias).gt(Expr::from(value))),
        FilterExpression::GreaterThanOrEqual { field, value } => {
            Some(get_column_expr(field, document, alias).gte(Expr::from(value)))
        }
        FilterExpression::LessThan { field, value } => Some(get_column_expr(field, document, alias).lt(Expr::from(value))),
        FilterExpression::LessThanOrEqual { field, value } => {
            Some(get_column_expr(field, document, alias).lte(Expr::from(value)))
        }
        FilterExpression::In { field, values } => {
            let exprs: Vec<Expr> = values.iter().map(Expr::from).collect();
            Some(get_column_expr(field, document, alias).is_in(exprs))
        }
        FilterExpression::NotIn { field, values } => {
            let exprs: Vec<Expr> = values.iter().map(Expr::from).collect();
            Some(get_column_expr(field, document, alias).is_not_in(exprs))
        }
        FilterExpression::Contains { field, value } => {
            let pattern = format!("%{}%", value);
            Some(get_column_expr(field, document, alias).like(pattern))
        }
        FilterExpression::StartsWith { field, value } => {
            let pattern = format!("{}%", value);
            Some(get_column_expr(field, document, alias).like(pattern))
        }
        FilterExpression::EndsWith { field, value } => {
            let pattern = format!("%{}", value);
            Some(get_column_expr(field, document, alias).like(pattern))
        }
        FilterExpression::IsNull { field } => Some(get_column_expr(field, document, alias).is_null()),
        FilterExpression::IsNotNull { field } => Some(get_column_expr(field, document, alias).is_not_null()),
        FilterExpression::HasRelation { .. } => None,
        _ => None,
    }
}

fn get_column_expr(field_path: &str, document: &DocumentType, alias: &str) -> Expr {
    let parts: Vec<&str> = field_path.split('.').collect();
    let base_field = parts[0];

    let column_name = if let Some(field) = document.fields.iter().find(|f| f.id.as_ref() == base_field) {
        field.id.normalized()
    } else {
        base_field.to_string()
    };

    Expr::col((alias, column_name))
}
