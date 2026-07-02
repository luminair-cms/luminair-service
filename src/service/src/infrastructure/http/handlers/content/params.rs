use std::collections::{HashSet, HashMap};
use std::str::FromStr;
use serde::Deserialize;
use luminair_common::{AttributeId, DocumentType, DocumentTypeApiId, entities::FieldType};
use url::form_urlencoded;

use crate::application::AppState;
use crate::domain::query::{DocumentStatus, FilterExpression, Sort, SortDirection};
use crate::domain::document::content::DomainValue;
use crate::infrastructure::http::api::ApiError;
use crate::infrastructure::http::handlers::content::PaginationParams;

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    /// A set of attribute IDs to populate in the response. If not provided, no relations will be populated.
    pub populate: Option<HashSet<String>>,
    /// Pagination parameters. Only eligible for find_all_documents query, not for find_by_id query.
    /// If not provided, defaults to page=1 and page_size=25.
    pub pagination: Option<PaginationParams>,
    /// Document publication status: "published" (default) or "draft"
    #[serde(default = "default_status")]
    pub status: String,
}

impl QueryParams {
    pub fn pagination_or_default(&self) -> (u16, u16) {
        self.pagination
            .as_ref()
            .map(|p| (p.page, p.page_size))
            .unwrap_or((1, 25))
    }
}

fn default_status() -> String {
    "published".to_string()
}

/// The wildcard token that, when supplied as the single `populate` value,
/// expands to every owning relation declared on the document type.
const POPULATE_WILDCARD: &str = "*";

/// Parse the `?status=` query parameter into a [`DocumentStatus`].
pub fn parse_status(s: &str) -> Result<DocumentStatus, ApiError> {
    match s {
        "draft" => Ok(DocumentStatus::Draft),
        "published" => Ok(DocumentStatus::Published),
        _ => Err(ApiError::UnprocessableEntity(
            "status must be 'published' (default) or 'draft'".to_string(),
        )),
    }
}

/// Convert the raw `?populate=` field set into a list of [`AttributeId`]s.
///
/// `populate=*` expands to every owning relation declared on `document_type`.
/// Returns `Ok(None)` when no populate parameter was supplied so the caller
/// can distinguish "do not populate anything" from "populate this empty set".
pub fn parse_populate(
    fields: Option<HashSet<String>>,
    document_type: &DocumentType,
) -> Result<Option<Vec<AttributeId>>, ApiError> {
    let Some(fields) = fields else {
        return Ok(None);
    };

    if fields.iter().any(|f| f == POPULATE_WILDCARD) {
        let expanded: Vec<AttributeId> = document_type
            .relations
            .iter()
            .filter(|rel| rel.relation_type.is_owning())
            .map(|rel| rel.id.clone())
            .collect();
        return Ok(Some(expanded));
    }

    let mut attributes = Vec::with_capacity(fields.len());
    for name in fields {
        let attr = AttributeId::try_new(&name).map_err(|_| {
            ApiError::UnprocessableEntity(format!("Invalid populate field: {}", name))
        })?;
        attributes.push(attr);
    }
    Ok(Some(attributes))
}

/// Resolve a `{api_type}` path segment to a registered [`DocumentType`].
pub fn resolve_document_type<S: AppState>(
    state: &S,
    api_type: &str,
) -> Result<&'static DocumentType, ApiError> {
    let api_id = DocumentTypeApiId::from_str(api_type)
        .map_err(|_| ApiError::UnprocessableEntity(format!("Invalid api_type: {}", api_type)))?;
    state
        .document_types()
        .lookup(&api_id)
        .ok_or(ApiError::NotFound)
}

fn parse_brackets(key: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = 0;
    while let Some(start) = key[current..].find('[') {
        let start_idx = current + start;
        if let Some(end) = key[start_idx..].find(']') {
            let end_idx = start_idx + end;
            parts.push(key[start_idx + 1..end_idx].to_string());
            current = end_idx + 1;
        } else {
            break;
        }
    }
    parts
}

fn parse_filter_value(val_str: &str, field_type: FieldType) -> Result<DomainValue, ApiError> {
    use std::str::FromStr;

    match field_type {
        FieldType::Integer(_) => {
            let i = i64::from_str(val_str).map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid integer filter value: {}", val_str))
            })?;
            Ok(DomainValue::Integer(i))
        }
        FieldType::Decimal { .. } => {
            let d = rust_decimal::Decimal::from_str(val_str).map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid decimal filter value: {}", val_str))
            })?;
            Ok(DomainValue::Decimal(d))
        }
        FieldType::Boolean => {
            let b = bool::from_str(val_str).map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid boolean filter value: {}", val_str))
            })?;
            Ok(DomainValue::Boolean(b))
        }
        FieldType::Date => {
            let d = chrono::NaiveDate::parse_from_str(val_str, "%Y-%m-%d").map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid date filter value (expected YYYY-MM-DD): {}", val_str))
            })?;
            Ok(DomainValue::Date(d))
        }
        FieldType::DateTime => {
            let dt = chrono::DateTime::parse_from_rfc3339(val_str).map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid datetime filter value (expected RFC 3339): {}", val_str))
            })?;
            Ok(DomainValue::DateTime(dt.with_timezone(&chrono::Utc)))
        }
        FieldType::Uuid => {
            let u = uuid::Uuid::from_str(val_str).map_err(|_| {
                ApiError::UnprocessableEntity(format!("Invalid UUID filter value: {}", val_str))
            })?;
            Ok(DomainValue::Uuid(u))
        }
        _ => Ok(DomainValue::Text(val_str.to_string())),
    }
}

fn build_filter_expr_for_operator(
    field_path: String,
    operator: &str,
    val_str: &str,
    field_type: FieldType,
) -> Result<FilterExpression, ApiError> {
    match operator {
        "$eq" | "" => {
            let val = parse_filter_value(val_str, field_type)?;
            Ok(FilterExpression::Equals { field: field_path, value: val })
        }
        "$ne" => {
            let val = parse_filter_value(val_str, field_type)?;
            Ok(FilterExpression::NotEquals { field: field_path, value: val })
        }
        "$gt" => {
            let val = parse_filter_value(val_str, field_type)?;
            Ok(FilterExpression::GreaterThan { field: field_path, value: val })
        }
        "$gte" => {
            let val = parse_filter_value(val_str, field_type)?;
            Ok(FilterExpression::GreaterThanOrEqual { field: field_path, value: val })
        }
        "$lt" => {
            let val = parse_filter_value(val_str, field_type)?;
            Ok(FilterExpression::LessThan { field: field_path, value: val })
        }
        "$lte" => {
            let val = parse_filter_value(val_str, field_type)?;
            Ok(FilterExpression::LessThanOrEqual { field: field_path, value: val })
        }
        "$contains" => {
            Ok(FilterExpression::Contains { field: field_path, value: val_str.to_string() })
        }
        "$startsWith" | "$starts_with" => {
            Ok(FilterExpression::StartsWith { field: field_path, value: val_str.to_string() })
        }
        "$endsWith" | "$ends_with" => {
            Ok(FilterExpression::EndsWith { field: field_path, value: val_str.to_string() })
        }
        "$null" => {
            let b = val_str.parse::<bool>().unwrap_or(true);
            if b {
                Ok(FilterExpression::IsNull { field: field_path })
            } else {
                Ok(FilterExpression::IsNotNull { field: field_path })
            }
        }
        "$notNull" | "$not_null" => {
            let b = val_str.parse::<bool>().unwrap_or(true);
            if b {
                Ok(FilterExpression::IsNotNull { field: field_path })
            } else {
                Ok(FilterExpression::IsNull { field: field_path })
            }
        }
        _ => Err(ApiError::UnprocessableEntity(format!("Unsupported filter operator: {}", operator))),
    }
}

pub fn parse_filters_and_sorts<S: AppState>(
    query_str: &str,
    document_type: &DocumentType,
    state: &S,
) -> Result<(FilterExpression, Option<HashMap<AttributeId, FilterExpression>>, Vec<Sort>), ApiError> {
    let query_pairs: Vec<(String, String)> = form_urlencoded::parse(query_str.as_bytes())
        .into_owned()
        .collect();

    let mut scalar_filters = Vec::new();
    let mut list_filters: HashMap<(String, String), Vec<String>> = HashMap::new();

    let mut relation_scalar_filters: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
    let mut relation_list_filters: HashMap<(String, String, String), Vec<String>> = HashMap::new();

    let mut sorts = Vec::new();

    for (key, value) in query_pairs {
        if key == "sort" {
            for item in value.split(',') {
                let parts: Vec<&str> = item.split(':').collect();
                if parts.is_empty() {
                    continue;
                }
                let field = parts[0].to_string();
                let direction = match parts.get(1).map(|d| d.to_ascii_lowercase()) {
                    Some(ref d) if d == "desc" => SortDirection::Descending,
                    _ => SortDirection::Ascending,
                };
                sorts.push(Sort { field, direction });
            }
            continue;
        }

        if !key.starts_with("filters") {
            continue;
        }

        let parts = parse_brackets(&key);
        if parts.is_empty() {
            continue;
        }

        let first_part = &parts[0];
        if document_type.relations.iter().any(|r| r.id.as_ref() == first_part) {
            // Relation filter
            let relation_name = first_part.clone();
            if parts.len() >= 2 {
                let rel_field = parts[1].clone();
                let opt_operator = parts.get(2).map(|s| s.as_str()).unwrap_or("$eq");

                if opt_operator == "$in" || opt_operator == "$notIn" || opt_operator == "$not_in" {
                    relation_list_filters
                        .entry((relation_name, rel_field, opt_operator.to_string()))
                        .or_default()
                        .push(value);
                } else if parts.len() >= 4 && (parts[2] == "$in" || parts[2] == "$notIn" || parts[2] == "$not_in") {
                    relation_list_filters
                        .entry((relation_name, rel_field, parts[2].to_string()))
                        .or_default()
                        .push(value);
                } else {
                    relation_scalar_filters
                        .entry(relation_name)
                        .or_default()
                        .push((rel_field, opt_operator.to_string(), value));
                }
            }
        } else {
            // Main field filter
            let is_localized = if let Some(field) = document_type.fields.iter().find(|f| f.id.as_ref() == first_part) {
                field.field_type == FieldType::LocalizedText
            } else {
                false
            };

            let (field_path, operator) = if is_localized && parts.len() >= 2 && !parts[1].starts_with('$') {
                let path = format!("{}.{}", first_part, parts[1]);
                let op = parts.get(2).map(|s| s.as_str()).unwrap_or("$eq");
                (path, op.to_string())
            } else {
                let op = parts.get(1).map(|s| s.as_str()).unwrap_or("$eq");
                (first_part.clone(), op.to_string())
            };

            let final_op = if parts.len() >= 3 && (parts[1] == "$in" || parts[1] == "$notIn" || parts[1] == "$not_in") {
                parts[1].clone()
            } else {
                operator
            };

            if final_op == "$in" || final_op == "$notIn" || final_op == "$not_in" {
                list_filters
                    .entry((field_path, final_op))
                    .or_default()
                    .push(value);
            } else {
                scalar_filters.push((field_path, final_op, value));
            }
        }
    }

    // Build main filters
    let mut main_filter = FilterExpression::None;

    for (field_path, operator, val_str) in scalar_filters {
        let base_field = field_path.split('.').next().unwrap_or(&field_path);
        let field_type = document_type
            .fields
            .iter()
            .find(|f| f.id.as_ref() == base_field)
            .map(|f| f.field_type)
            .unwrap_or(FieldType::Text);

        let expr = build_filter_expr_for_operator(field_path, &operator, &val_str, field_type)?;
        main_filter = match main_filter {
            FilterExpression::None => expr,
            _ => FilterExpression::And(Box::new(main_filter), Box::new(expr)),
        };
    }

    for ((field_path, operator), val_strs) in list_filters {
        let base_field = field_path.split('.').next().unwrap_or(&field_path);
        let field_type = document_type
            .fields
            .iter()
            .find(|f| f.id.as_ref() == base_field)
            .map(|f| f.field_type)
            .unwrap_or(FieldType::Text);

        let mut domain_values = Vec::new();
        for val_str in val_strs {
            domain_values.push(parse_filter_value(&val_str, field_type)?);
        }

        let expr = if operator == "$in" {
            FilterExpression::In { field: field_path, values: domain_values }
        } else {
            FilterExpression::NotIn { field: field_path, values: domain_values }
        };

        main_filter = match main_filter {
            FilterExpression::None => expr,
            _ => FilterExpression::And(Box::new(main_filter), Box::new(expr)),
        };
    }

    // Build relation filters
    let mut populate_filters = HashMap::new();

    for (relation_name, scalars) in relation_scalar_filters {
        let rel_attr = AttributeId::try_new(&relation_name).map_err(|_| {
            ApiError::UnprocessableEntity(format!("Invalid relation: {}", relation_name))
        })?;

        let rel_meta = document_type.relations.iter().find(|r| r.id == rel_attr).ok_or_else(|| {
            ApiError::UnprocessableEntity(format!("Relation not found: {}", relation_name))
        })?;

        let target_type = state
            .document_types()
            .get(&rel_meta.target)
            .ok_or(ApiError::NotFound)?;

        let mut rel_filter = FilterExpression::None;
        for (field, operator, val_str) in scalars {
            let field_type = target_type
                .fields
                .iter()
                .find(|f| f.id.as_ref() == field)
                .map(|f| f.field_type)
                .unwrap_or(FieldType::Text);

            let expr = build_filter_expr_for_operator(field, &operator, &val_str, field_type)?;
            rel_filter = match rel_filter {
                FilterExpression::None => expr,
                _ => FilterExpression::And(Box::new(rel_filter), Box::new(expr)),
            };
        }
        populate_filters.insert(rel_attr, rel_filter);
    }

    for ((relation_name, field, operator), val_strs) in relation_list_filters {
        let rel_attr = AttributeId::try_new(&relation_name).map_err(|_| {
            ApiError::UnprocessableEntity(format!("Invalid relation: {}", relation_name))
        })?;

        let rel_meta = document_type.relations.iter().find(|r| r.id == rel_attr).ok_or_else(|| {
            ApiError::UnprocessableEntity(format!("Relation not found: {}", relation_name))
        })?;

        let target_type = state
            .document_types()
            .get(&rel_meta.target)
            .ok_or(ApiError::NotFound)?;

        let field_type = target_type
            .fields
            .iter()
            .find(|f| f.id.as_ref() == field)
            .map(|f| f.field_type)
            .unwrap_or(FieldType::Text);

        let mut domain_values = Vec::new();
        for val_str in val_strs {
            domain_values.push(parse_filter_value(&val_str, field_type)?);
        }

        let expr = if operator == "$in" {
            FilterExpression::In { field, values: domain_values }
        } else {
            FilterExpression::NotIn { field, values: domain_values }
        };

        let current_filter = populate_filters.entry(rel_attr).or_insert(FilterExpression::None);
        let updated_filter = match current_filter {
            FilterExpression::None => expr,
            _ => FilterExpression::And(Box::new(std::mem::replace(current_filter, FilterExpression::None)), Box::new(expr)),
        };
        *current_filter = updated_filter;
    }

    let pop_filters = if populate_filters.is_empty() {
        None
    } else {
        Some(populate_filters)
    };

    Ok((main_filter, pop_filters, sorts))
}
