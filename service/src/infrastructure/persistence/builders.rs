use luminair_common::persistence::TableNameProvider;
use luminair_common::{
    CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType, ID_FIELD_NAME,
    INVERSE_ID_FIELD_NAME, OWNING_ID_FIELD_NAME, PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME,
    REVISION_FIELD_NAME, UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME,
};
use crate::domain::document::DatabaseRowId;
use crate::domain::repository::query::{FilterExpression, SortDirection, DocumentInstanceQuery};
use sea_query::extension::postgres::PgExpr;
use sea_query::{
    ColumnRef, Condition, DynIden, Expr, ExprTrait, Iden, InsertStatement, IntoColumnRef, JoinType, Order,
    PostgresQueryBuilder, Query, SimpleExpr, TableRef,
};
use sea_query_sqlx::{SqlxBinder, SqlxValues};
use std::convert::Into;
use uuid::Uuid;

pub fn query_find_document_by_id(
    document: &DocumentType,
    id: Uuid,
    query: &DocumentInstanceQuery,
) -> (String, SqlxValues) {
    let table: TableNameProvider = document.into();
    let columns = main_select_columns(document);

    let document_id_column = Expr::col(("m", DOCUMENT_ID_FIELD_NAME));

    let mut select = Query::select();
    select.columns(columns).from(table).and_where(document_id_column.eq(id));

    if document.has_draft_and_publish() && !query.include_drafts {
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

    if document.has_draft_and_publish() && !query.include_drafts {
        select.and_where(Expr::col(("m", PUBLISHED_FIELD_NAME)).is_not_null());
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

fn build_filter_expr(filter: &FilterExpression, document: &DocumentType) -> Option<SimpleExpr> {
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
        FilterExpression::IsNull { field } => {
            Some(get_column_expr(field, document).is_null())
        }
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

fn get_column_expr(field_path: &str, document: &DocumentType) -> SimpleExpr {
    let parts: Vec<&str> = field_path.split('.').collect();
    let base_field = parts[0];
    
    // Check if it's a known field to get the correct column name (normalized)
    let column_name = if let Some(field) = document.fields.iter().find(|f| f.id.as_ref() == base_field) {
        field.id.normalized()
    } else {
        base_field.to_string()
    };

    let col = Expr::col(("m", column_name));

    if parts.len() > 1 {
        let mut expr = col.into_simple_expr();
        for part in &parts[1..] {
            expr = PgExpr::json_get_path_text(expr, vec![part.to_string()]);
        }
        expr
    } else {
        col.into_simple_expr()
    }
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

pub fn insert_relation_entry(
    document: &DocumentType,
    relation_attr: &luminair_common::AttributeId,
    owning_id: DatabaseRowId,
    inverse_id: DatabaseRowId,
) -> (String, SqlxValues) {
    let relation_table: TableNameProvider = (document, relation_attr).into();

    let columns: Vec<DynIden> = vec![
        OWNING_ID_FIELD_NAME.into(),
        INVERSE_ID_FIELD_NAME.into(),
    ];

    Query::insert()
        .into_table(relation_table)
        .columns(columns)
        .values_panic(vec![owning_id.0.into(), inverse_id.0.into()])
        .build_sqlx(PostgresQueryBuilder)
}

pub fn delete_relation_entry(
    document: &DocumentType,
    relation_attr: &luminair_common::AttributeId,
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
