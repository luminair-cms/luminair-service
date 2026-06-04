use sea_query::{Condition, Expr, ExprTrait, Order, PostgresQueryBuilder, Query};
use sea_query_sqlx::{SqlxBinder, SqlxValues};
use uuid::Uuid;
use luminair_common::{DocumentType, DOCUMENT_ID_FIELD_NAME, ID_FIELD_NAME, PUBLISHED_FIELD_NAME};
use luminair_common::persistence::TableNameProvider;
use crate::domain::query::{DocumentInstanceQuery, DocumentStatus, FilterExpression, SortDirection};
use crate::infrastructure::persistence::builders::main_select_columns;

pub fn query_find_document_by_id(
    document: &DocumentType,
    id: Uuid,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();
    let columns = main_select_columns(document);

    let document_id_column = Expr::col(("m", DOCUMENT_ID_FIELD_NAME));

    let mut select = Query::select();
    select
        .columns(columns)
        .from(table)
        .and_where(document_id_column.eq(id));

    if document.has_draft_and_publish() && query.status == DocumentStatus::Published {
        // for find-by-id: only published OR published+draft
        select.and_where(Expr::col(("m", PUBLISHED_FIELD_NAME)).is_not_null());
    }

    // Even for find_by_id, we might want to apply filters (e.g. status)
    if let Some(condition) = build_condition(&query.filter, document) {
        select.cond_where(condition);
    }

    select.build_sqlx(PostgresQueryBuilder)
}

pub fn query_find_document_by_criteria(
    document: &DocumentType,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();
    let columns = main_select_columns(document);

    let mut select = Query::select();
    select.columns(columns).from(table);

    if document.has_draft_and_publish() {
        // for find by example: only published OR only draft
        if query.status == DocumentStatus::Published {
            select.and_where(Expr::col(("m", PUBLISHED_FIELD_NAME)).is_not_null());
        } else {
            select.and_where(Expr::col(("m", PUBLISHED_FIELD_NAME)).is_null());
        }
    }

    // Apply custom filters
    if let Some(condition) = build_condition(&query.filter, document) {
        select.cond_where(condition);
    }

    // Apply sorting
    for sort in &query.sort {
        let col = get_column_expr(&sort.field, document);
        let order = match sort.direction {
            SortDirection::Ascending => Order::Asc,
            SortDirection::Descending => Order::Desc,
        };
        select.order_by_expr(col, order);
    }

    // Apply pagination
    if let Some(limit) = query.limit {
        select.limit(limit as u64);
    }
    if let Some(offset) = query.offset {
        select.offset(offset as u64);
    }

    select.build_sqlx(PostgresQueryBuilder)
}

/// SELECT COUNT(*) FROM {table} — with the same WHERE conditions as `query_find_document_by_criteria`.
/// Used for accurate pagination metadata.
pub fn query_count_documents(
    document: &DocumentType,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();

    let mut select = Query::select();
    select
        .expr(Expr::col(("m", ID_FIELD_NAME)).count())
        .from(table);

    if document.has_draft_and_publish() {
        if query.status == DocumentStatus::Published {
            select.and_where(Expr::col(("m", PUBLISHED_FIELD_NAME)).is_not_null());
        } else {
            select.and_where(Expr::col(("m", PUBLISHED_FIELD_NAME)).is_null());
        }
    }

    if let Some(condition) = build_condition(&query.filter, document) {
        select.cond_where(condition);
    }

    select.build_sqlx(PostgresQueryBuilder)
}

fn build_condition(filter: &FilterExpression, document: &DocumentType) -> Option<Condition> {
    match filter {
        FilterExpression::None => None,
        FilterExpression::And(left, right) => {
            let left_cond = build_condition(left, document);
            let right_cond = build_condition(right, document);
            match (left_cond, right_cond) {
                (Some(l), Some(r)) => Some(Condition::all().add(l).add(r)),
                (Some(l), None) => Some(l),
                (None, Some(r)) => Some(r),
                (None, None) => None,
            }
        }
        FilterExpression::Or(left, right) => {
            let left_cond = build_condition(left, document);
            let right_cond = build_condition(right, document);
            match (left_cond, right_cond) {
                (Some(l), Some(r)) => Some(Condition::any().add(l).add(r)),
                (Some(l), None) => Some(l),
                (None, Some(r)) => Some(r),
                (None, None) => None,
            }
        }
        _ => {
            if let Some(expr) = build_filter_expr(filter, document) {
                Some(Condition::all().add(expr))
            } else {
                None
            }
        }
    }
}


fn build_filter_expr(filter: &FilterExpression, document: &DocumentType) -> Option<Expr> {
    match filter {
        FilterExpression::Equals { field, value } => {
            Some(get_column_expr(field, document).eq(Expr::from(value)))
        }
        FilterExpression::NotEquals { field, value } => {
            Some(get_column_expr(field, document).ne(Expr::from(value)))
        }
        FilterExpression::GreaterThan { field, value } => {
            Some(get_column_expr(field, document).gt(Expr::from(value)))
        }
        FilterExpression::GreaterThanOrEqual { field, value } => {
            Some(get_column_expr(field, document).gte(Expr::from(value)))
        }
        FilterExpression::LessThan { field, value } => {
            Some(get_column_expr(field, document).lt(Expr::from(value)))
        }
        FilterExpression::LessThanOrEqual { field, value } => {
            Some(get_column_expr(field, document).lte(Expr::from(value)))
        }
        FilterExpression::In { field, values } => {
            let exprs: Vec<Expr> = values.iter().map(Expr::from).collect();
            Some(get_column_expr(field, document).is_in(exprs))
        }
        FilterExpression::NotIn { field, values } => {
            let exprs: Vec<Expr> = values.iter().map(Expr::from).collect();
            Some(get_column_expr(field, document).is_not_in(exprs))
        }
        FilterExpression::Contains { field, value } => {
            let pattern = format!("%{}%", value);
            Some(get_column_expr(field, document).like(pattern))
        }
        FilterExpression::StartsWith { field, value } => {
            let pattern = format!("{}%", value);
            Some(get_column_expr(field, document).like(pattern))
        }
        FilterExpression::EndsWith { field, value } => {
            let pattern = format!("%{}", value);
            Some(get_column_expr(field, document).like(pattern))
        }
        FilterExpression::IsNull { field } => Some(get_column_expr(field, document).is_null()),
        FilterExpression::IsNotNull { field } => {
            Some(get_column_expr(field, document).is_not_null())
        }
        FilterExpression::HasRelation { .. } => {
            // Relation filtering requires joins, which is not yet implemented in build_filter_expr.
            // This would require modifying the Query object itself to add JOINS.
            None
        }
        _ => None,
    }
}

fn get_column_expr(field_path: &str, document: &DocumentType) -> Expr {
    // TODO: name1.name2.name3
    let parts: Vec<&str> = field_path.split('.').collect();
    let base_field = parts[0];

    // Check if it's a known field to get the correct column name (normalized)
    let column_name =
        if let Some(field) = document.fields.iter().find(|f| f.id.as_ref() == base_field) {
            field.id.normalized()
        } else {
            base_field.to_string()
        };

    let col = Expr::col(("m", column_name));
    col
}
