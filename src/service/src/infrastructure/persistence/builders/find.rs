use crate::domain::query::{
    DocumentInstanceQuery, DocumentStatus, FilterExpression, SortDirection,
};

use luminair_common::persistence::TableNameProviderConstructor;
use luminair_common::{
    CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType,
    PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, STATUS_FIELD_NAME,
    UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME,
};
use sea_query::{
    Alias, ColumnRef, Condition, Expr, ExprTrait, Order, PostgresQueryBuilder, Query, SelectStatement, TableRef
};
use sea_query_sqlx::{SqlxBinder, SqlxValues};
use uuid::Uuid;

/**
 * Create query for find ONE document by document_id + status
 * THERE IS NO HISTORY IN MVP,
 * so in snapshots only one record exists - last published version

 if query.status == DocumentStatus::Published:

 SELECT
    m.document_id,
    m.revision,
    0 as version,
    'PUBLISHED' as status,
    m.created_at,
    m.updated_at,
    m.created_by_id,
    m.updated_by_id,
    m.published_at,
    m.published_by_id,
    m.title,
    m.body
FROM article_snapshots m
WHERE m.document_id = $1

if query.status == DocumentStatus::Draft:

SELECT
    m.document_id,
    m.revision,
    m.version,
    m.status,
    m.created_at,
    m.updated_at,
    m.created_by_id,
    m.updated_by_id,
    m.published_at,
    m.published_by_id,
    m.title,
    m.body
FROM articles m
WHERE m.document_id = $1;
 */
pub fn query_find_document_by_id(
    document: &DocumentType,
    id: Uuid,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    let mut select = main_document_select(document, query.status);
    select.and_where(Expr::col(("m", DOCUMENT_ID_FIELD_NAME)).eq(id));

    if let Some(condition) = build_condition(&query.filter, document, "m") {
        select.cond_where(condition);
    }

    select.build_sqlx(PostgresQueryBuilder)
}

pub fn query_find_document_by_criteria(
    document: &DocumentType,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    let mut select = main_document_select(document, query.status);

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

fn main_document_select<'a>(
    document: &'a DocumentType,
    status: DocumentStatus,
) -> SelectStatement {
    let (table_ref, status_expr, version_expr) = if status == DocumentStatus::Published {
        let table_ref = document.snapshot_table();
        (
            TableRef::from(table_ref),
            Expr::cust("'PUBLISHED'"),
            Expr::cust("0"),
        )
    } else {
        let table_ref = document.main_table();
        let status_column: ColumnRef = ("m", STATUS_FIELD_NAME).into();
        let version_column: ColumnRef = ("m", VERSION_FIELD_NAME).into();

        (
            TableRef::from(table_ref),
            Expr::col(status_column),
            Expr::col(version_column),
        )
    };

    let mut select = Query::select();
    select.from(table_ref);

    // Add regular columns via .columns()
    select.columns(common_select_columns(document));

    // Add typed/custom expressions via .expr_as()
    select.expr_as(version_expr, Alias::new("version"));
    select.expr_as(status_expr, Alias::new("status"));

    select
}

fn common_select_columns(document: &DocumentType) -> Vec<ColumnRef> {
    let mut columns: Vec<ColumnRef> = vec![
        ("m", DOCUMENT_ID_FIELD_NAME).into(),
        ("m", CREATED_FIELD_NAME).into(),
        ("m", UPDATED_FIELD_NAME).into(),
        ("m", CREATED_BY_FIELD_NAME).into(),
        ("m", UPDATED_BY_FIELD_NAME).into(),
        ("s", PUBLISHED_FIELD_NAME).into(),
        ("s", PUBLISHED_BY_FIELD_NAME).into(),
        ("s", REVISION_FIELD_NAME).into(),
    ];

    for field in &document.fields {
        columns.push(("m", field.id.normalized()).into());
    }

    columns
}

pub fn query_count_documents(
    document: &DocumentType,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    let table_ref = if query.status == DocumentStatus::Published {
        document.snapshot_table()
    } else {
        document.main_table()
    };

    let mut select = Query::select();
    select
        .expr_as(Expr::cust("COUNT(DISTINCT m.document_id)"), Alias::new("count"))
        .from(table_ref);

    if let Some(condition) = build_condition(&query.filter, document, "m") {
        select.cond_where(condition);
    }

    select.build_sqlx(PostgresQueryBuilder)
}

fn build_condition(
    filter: &FilterExpression,
    document: &DocumentType,
    alias: &str,
) -> Option<Condition> {
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

fn build_filter_expr(
    filter: &FilterExpression,
    document: &DocumentType,
    alias: &str,
) -> Option<Expr> {
    match filter {
        FilterExpression::Equals { field, value } => {
            Some(get_column_expr(field, document, alias).eq(Expr::from(value)))
        }
        FilterExpression::NotEquals { field, value } => {
            Some(get_column_expr(field, document, alias).ne(Expr::from(value)))
        }
        FilterExpression::GreaterThan { field, value } => {
            Some(get_column_expr(field, document, alias).gt(Expr::from(value)))
        }
        FilterExpression::GreaterThanOrEqual { field, value } => {
            Some(get_column_expr(field, document, alias).gte(Expr::from(value)))
        }
        FilterExpression::LessThan { field, value } => {
            Some(get_column_expr(field, document, alias).lt(Expr::from(value)))
        }
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
        FilterExpression::IsNull { field } => {
            Some(get_column_expr(field, document, alias).is_null())
        }
        FilterExpression::IsNotNull { field } => {
            Some(get_column_expr(field, document, alias).is_not_null())
        }
        FilterExpression::HasRelation { .. } => None,
        _ => None,
    }
}

fn get_column_expr(field_path: &str, document: &DocumentType, alias: &str) -> Expr {
    let parts: Vec<&str> = field_path.split('.').collect();
    let base_field = parts[0];

    let column_name =
        if let Some(field) = document.fields.iter().find(|f| f.id.as_ref() == base_field) {
            field.id.normalized()
        } else {
            base_field.to_string()
        };

    Expr::col((alias.to_owned(), column_name))
}
