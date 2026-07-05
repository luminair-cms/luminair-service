use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use luminair_common::{AttributeId, DocumentType, DocumentTypesRegistry, entities::FieldType};
use serde_json::Value;

use crate::domain::document::content::DomainValue;
use crate::domain::query::{DocumentStatus, FilterExpression, Sort, SortDirection};
use crate::infrastructure::http::api::ApiError;

// ─── Constants ────────────────────────────────────────────────────────────────

/// The wildcard token that, when supplied as the single `populate` value,
/// expands to every owning relation declared on the document type.
const POPULATE_WILDCARD: &str = "*";

// ─── Public structs ───────────────────────────────────────────────────────────

/// Schema-agnostic representation of every bracket query parameter.
///
/// Produced by [`parse_raw_query`] without any domain knowledge.
/// Use [`parse_query`] to validate and resolve it against a [`DocumentType`].
pub(super) struct RawQueryParams {
    /// `?populate=*` / `?populate[]=field` / `?populate=field`
    pub populate: Option<HashSet<String>>,
    /// `?pagination[page]=N&pagination[pageSize]=M`
    pub pagination: (u16, u16),
    /// `?status=draft|published` — raw string, not yet validated against the domain enum
    pub status: String,
    /// `?sort=field:asc,other:desc`
    pub sorts: Vec<(String, SortDirection)>,
    /// `?filters[...]` — the nested JSON subtree, kept opaque for the validation layer
    pub filters: Option<Value>,
}

/// Fully resolved, domain-validated query parameters ready for the application layer.
pub struct DocumentQuery {
    pub populate: Option<Vec<AttributeId>>,
    pub pagination: (u16, u16),
    pub status: DocumentStatus,
    pub filter: FilterExpression,
    pub populate_filters: Option<HashMap<AttributeId, FilterExpression>>,
    pub sorts: Vec<Sort>,
}

// ─── Public functions ─────────────────────────────────────────────────────────

/// Parse a nested query-string map into [`RawQueryParams`] with no schema knowledge.
///
/// This is a pure structural transformation: it extracts the well-known top-level
/// keys (`populate`, `pagination`, `status`, `sort`, `filters`) from the already-
/// decoded bracket map and returns them in typed form, without any domain validation.
pub(super) fn parse_raw_query(query_map: &serde_json::Map<String, Value>) -> RawQueryParams {
    // populate
    let populate = match query_map.get("populate") {
        Some(Value::String(s)) => {
            let mut set = HashSet::new();
            set.insert(s.clone());
            Some(set)
        }
        Some(Value::Array(arr)) => {
            let set = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<HashSet<_>>();
            Some(set)
        }
        _ => None,
    };

    // pagination
    let pagination = if let Some(Value::Object(pag_map)) = query_map.get("pagination") {
        let page = pag_map
            .get("page")
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<u16>().ok())
                    .or_else(|| v.as_u64().map(|n| n as u16))
            })
            .unwrap_or(1);
        let page_size = pag_map
            .get("pageSize")
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<u16>().ok())
                    .or_else(|| v.as_u64().map(|n| n as u16))
            })
            .unwrap_or(25);
        (page, page_size)
    } else {
        (1, 25)
    };

    // status
    let status = query_map
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("published")
        .to_string();

    // sorts
    let sorts = query_map
        .get("sort")
        .and_then(|v| v.as_str())
        .map(|sort_val| {
            sort_val
                .split(',')
                .filter(|item| !item.is_empty())
                .map(|item| {
                    let mut parts = item.splitn(2, ':');
                    let field = parts.next().unwrap_or("").to_string();
                    let direction = match parts.next().map(|d| d.to_ascii_lowercase()).as_deref() {
                        Some("desc") => SortDirection::Descending,
                        _ => SortDirection::Ascending,
                    };
                    (field, direction)
                })
                .collect()
        })
        .unwrap_or_default();

    // filters — keep as opaque JSON value for the validation layer
    let filters = query_map.get("filters").cloned();

    RawQueryParams { populate, pagination, status, sorts, filters }
}

/// Parse and validate all query parameters against the given [`DocumentType`] schema.
///
/// Internally calls [`parse_raw_query`] for structural parsing, then validates and
/// resolves each field using domain knowledge from `document_type` and `registry`.
pub fn parse_query(
    query_map: &serde_json::Map<String, Value>,
    document_type: &DocumentType,
    registry: &dyn DocumentTypesRegistry,
) -> Result<DocumentQuery, ApiError> {
    let raw = parse_raw_query(query_map);

    let status = parse_status(&raw.status)?;
    let populate = resolve_populate(raw.populate, document_type)?;
    let sorts = raw
        .sorts
        .into_iter()
        .map(|(field, direction)| Sort { field, direction })
        .collect();
    let (filter, populate_filters) = resolve_filters(raw.filters, document_type, registry)?;

    Ok(DocumentQuery {
        populate,
        pagination: raw.pagination,
        status,
        filter,
        populate_filters,
        sorts,
    })
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Validate a raw `status` string into the domain [`DocumentStatus`] enum.
fn parse_status(s: &str) -> Result<DocumentStatus, ApiError> {
    match s {
        "draft" => Ok(DocumentStatus::Draft),
        "published" => Ok(DocumentStatus::Published),
        _ => Err(ApiError::UnprocessableEntity(
            "status must be 'published' (default) or 'draft'".to_string(),
        )),
    }
}

/// Resolve raw populate field names into validated [`AttributeId`]s.
///
/// The wildcard `*` is expanded to every owning relation on the document type.
fn resolve_populate(
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

/// Entry point for filter resolution: dispatches the raw filter JSON value into the
/// recursive resolver and returns the main filter and per-relation populate filters.
fn resolve_filters(
    filters: Option<Value>,
    document_type: &DocumentType,
    registry: &dyn DocumentTypesRegistry,
) -> Result<(FilterExpression, Option<HashMap<AttributeId, FilterExpression>>), ApiError> {
    let mut main_filter = FilterExpression::None;
    let mut populate_filters: HashMap<AttributeId, FilterExpression> = HashMap::new();

    if let Some(filters_val) = filters {
        parse_filters_recursive(
            &filters_val,
            "",
            document_type,
            registry,
            &mut main_filter,
            &mut populate_filters,
            None,
        )?;
    }

    let populate_filters = if populate_filters.is_empty() {
        None
    } else {
        Some(populate_filters)
    };

    Ok((main_filter, populate_filters))
}

/// Recursively walk the nested filter JSON value, resolving each node to a
/// [`FilterExpression`] using the document type schema and relation registry.
fn parse_filters_recursive(
    value: &Value,
    current_path: &str,
    document_type: &DocumentType,
    registry: &dyn DocumentTypesRegistry,
    main_filter: &mut FilterExpression,
    relation_filters: &mut HashMap<AttributeId, FilterExpression>,
    current_relation: Option<&AttributeId>,
) -> Result<(), ApiError> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key.starts_with('$') {
                    // Operator leaf node — build an expression for the current field path
                    let base_field = current_path.split('.').next().unwrap_or(current_path);
                    let field_type = document_type
                        .fields
                        .iter()
                        .find(|f| f.id.as_ref() == base_field)
                        .map(|f| f.field_type)
                        .unwrap_or(FieldType::Text);

                    let expr = build_filter_expr_for_json_value(
                        current_path.to_string(),
                        key,
                        child,
                        field_type,
                    )?;

                    accumulate_filter(main_filter, relation_filters, current_relation, expr);
                } else if let Some(rel) =
                    document_type.relations.iter().find(|r| r.id.as_ref() == key)
                {
                    // Relation node — switch document type context to the relation target
                    let target_type = registry.get(&rel.target).ok_or(ApiError::NotFound)?;

                    parse_filters_recursive(
                        child,
                        "",
                        target_type,
                        registry,
                        main_filter,
                        relation_filters,
                        Some(&rel.id),
                    )?;
                } else {
                    // Standard field key or locale code — extend the path and recurse
                    let new_path = if current_path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", current_path, key)
                    };

                    parse_filters_recursive(
                        child,
                        &new_path,
                        document_type,
                        registry,
                        main_filter,
                        relation_filters,
                        current_relation,
                    )?;
                }
            }
        }
        _ => {
            // Bare value with no operator key — default to $eq
            let base_field = current_path.split('.').next().unwrap_or(current_path);
            let field_type = document_type
                .fields
                .iter()
                .find(|f| f.id.as_ref() == base_field)
                .map(|f| f.field_type)
                .unwrap_or(FieldType::Text);

            let expr = build_filter_expr_for_json_value(
                current_path.to_string(),
                "$eq",
                value,
                field_type,
            )?;

            accumulate_filter(main_filter, relation_filters, current_relation, expr);
        }
    }
    Ok(())
}

/// Dispatch a single `(field_path, operator, JSON value)` triple to the correct
/// [`FilterExpression`] variant, handling list operators (`$in`, `$notIn`) separately.
fn build_filter_expr_for_json_value(
    field_path: String,
    operator: &str,
    value: &Value,
    field_type: FieldType,
) -> Result<FilterExpression, ApiError> {
    if operator == "$in" || operator == "$notIn" || operator == "$not_in" {
        let mut domain_values = Vec::new();
        match value {
            Value::Array(arr) => {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        domain_values.push(parse_filter_value(s, field_type)?);
                    }
                }
            }
            Value::String(s) => {
                domain_values.push(parse_filter_value(s, field_type)?);
            }
            _ => {
                return Err(ApiError::UnprocessableEntity(format!(
                    "Expected array or string for operator {}",
                    operator
                )))
            }
        }

        if operator == "$in" {
            Ok(FilterExpression::In { field: field_path, values: domain_values })
        } else {
            Ok(FilterExpression::NotIn { field: field_path, values: domain_values })
        }
    } else {
        let val_str = match value {
            Value::String(s) => s.as_str(),
            Value::Number(n) => {
                return parse_filter_value(&n.to_string(), field_type).map(|val| match operator {
                    "$ne" => FilterExpression::NotEquals { field: field_path.clone(), value: val },
                    "$gt" => FilterExpression::GreaterThan { field: field_path.clone(), value: val },
                    "$gte" => FilterExpression::GreaterThanOrEqual { field: field_path.clone(), value: val },
                    "$lt" => FilterExpression::LessThan { field: field_path.clone(), value: val },
                    "$lte" => FilterExpression::LessThanOrEqual { field: field_path.clone(), value: val },
                    _ => FilterExpression::Equals { field: field_path.clone(), value: val },
                });
            }
            Value::Bool(b) => {
                if *b { "true" } else { "false" }
            }
            _ => {
                return Err(ApiError::UnprocessableEntity(format!(
                    "Expected scalar value for operator {}",
                    operator
                )))
            }
        };

        build_filter_expr_for_operator(field_path, operator, val_str, field_type)
    }
}

/// Map a comparison operator string + scalar string value to a [`FilterExpression`].
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
        "$contains" => Ok(FilterExpression::Contains { field: field_path, value: val_str.to_string() }),
        "$startsWith" | "$starts_with" => {
            Ok(FilterExpression::StartsWith { field: field_path, value: val_str.to_string() })
        }
        "$endsWith" | "$ends_with" => {
            Ok(FilterExpression::EndsWith { field: field_path, value: val_str.to_string() })
        }
        "$null" => {
            if val_str.parse::<bool>().unwrap_or(true) {
                Ok(FilterExpression::IsNull { field: field_path })
            } else {
                Ok(FilterExpression::IsNotNull { field: field_path })
            }
        }
        "$notNull" | "$not_null" => {
            if val_str.parse::<bool>().unwrap_or(true) {
                Ok(FilterExpression::IsNotNull { field: field_path })
            } else {
                Ok(FilterExpression::IsNull { field: field_path })
            }
        }
        _ => Err(ApiError::UnprocessableEntity(format!(
            "Unsupported filter operator: {}",
            operator
        ))),
    }
}

/// Parse a raw scalar string into a typed [`DomainValue`] based on the field's schema type.
fn parse_filter_value(val_str: &str, field_type: FieldType) -> Result<DomainValue, ApiError> {
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
                ApiError::UnprocessableEntity(format!(
                    "Invalid date filter value (expected YYYY-MM-DD): {}",
                    val_str
                ))
            })?;
            Ok(DomainValue::Date(d))
        }
        FieldType::DateTime => {
            let dt = chrono::DateTime::parse_from_rfc3339(val_str).map_err(|_| {
                ApiError::UnprocessableEntity(format!(
                    "Invalid datetime filter value (expected RFC 3339): {}",
                    val_str
                ))
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

/// AND a new filter expression into the appropriate accumulator slot
/// (main or per-relation), building up an `And` chain as needed.
fn accumulate_filter(
    main_filter: &mut FilterExpression,
    relation_filters: &mut HashMap<AttributeId, FilterExpression>,
    current_relation: Option<&AttributeId>,
    expr: FilterExpression,
) {
    let target = if let Some(rel_id) = current_relation {
        relation_filters
            .entry(rel_id.clone())
            .or_insert(FilterExpression::None)
    } else {
        main_filter
    };

    let updated = match std::mem::replace(target, FilterExpression::None) {
        FilterExpression::None => expr,
        existing => FilterExpression::And(Box::new(existing), Box::new(expr)),
    };
    *target = updated;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::http::querystring::parse_query_to_json;
    use luminair_common::entities::{
        DocumentField, DocumentKind, DocumentRelation, DocumentTitle, DocumentTypeInfo, RelationType,
    };
    use luminair_common::{DocumentTypeApiId, DocumentTypeId};

    #[derive(Debug)]
    struct MockRegistry {
        types: HashMap<DocumentTypeId, &'static DocumentType>,
    }

    impl DocumentTypesRegistry for MockRegistry {
        fn iterate(&self) -> Box<dyn Iterator<Item = &DocumentType> + '_> {
            panic!("unimplemented")
        }
        fn get(&self, id: &DocumentTypeId) -> Option<&DocumentType> {
            self.types.get(id).map(|r| *r)
        }
        fn lookup(&self, _api_id: &DocumentTypeApiId) -> Option<&DocumentType> {
            None
        }
    }

    #[test]
    fn test_parse_query_filters() {
        let dt_category: &'static DocumentType = Box::leak(Box::new(DocumentType {
            id: DocumentTypeId::try_new("category").unwrap(),
            kind: DocumentKind::Collection,
            info: DocumentTypeInfo {
                title: DocumentTitle::try_new("Category").unwrap(),
                singular_name: DocumentTypeId::try_new("category").unwrap(),
                plural_name: DocumentTypeId::try_new("categories").unwrap(),
                description: None,
            },
            options: None,
            fields: HashSet::from([DocumentField {
                id: AttributeId::try_new("slug").unwrap(),
                field_type: FieldType::Text,
                constraints: HashSet::new(),
                required: false,
                unique: false,
            }]),
            relations: HashSet::new(),
        }));

        let dt_restaurant: &'static DocumentType = Box::leak(Box::new(DocumentType {
            id: DocumentTypeId::try_new("restaurant").unwrap(),
            kind: DocumentKind::Collection,
            info: DocumentTypeInfo {
                title: DocumentTitle::try_new("Restaurant").unwrap(),
                singular_name: DocumentTypeId::try_new("restaurant").unwrap(),
                plural_name: DocumentTypeId::try_new("restaurants").unwrap(),
                description: None,
            },
            options: None,
            fields: HashSet::from([
                DocumentField {
                    id: AttributeId::try_new("title").unwrap(),
                    field_type: FieldType::Text,
                    constraints: HashSet::new(),
                    required: false,
                    unique: false,
                },
                DocumentField {
                    id: AttributeId::try_new("description").unwrap(),
                    field_type: FieldType::LocalizedText,
                    constraints: HashSet::new(),
                    required: false,
                    unique: false,
                },
            ]),
            relations: HashSet::from([DocumentRelation {
                id: AttributeId::try_new("category").unwrap(),
                target: DocumentTypeId::try_new("category").unwrap(),
                relation_type: RelationType::HasOne,
            }]),
        }));

        let mut types = HashMap::new();
        types.insert(dt_category.id.clone(), dt_category);
        types.insert(dt_restaurant.id.clone(), dt_restaurant);
        let registry = MockRegistry { types };

        let query = "filters[title][$eq]=hello\
            &filters[description][en][$contains]=world\
            &filters[category][slug][$eq]=italian\
            &sort=title:asc\
            &status=draft\
            &pagination[page]=2\
            &pagination[pageSize]=10";
        let query_map = parse_query_to_json(query).unwrap();

        let q = parse_query(&query_map, dt_restaurant, &registry).unwrap();

        assert_eq!(q.pagination, (2, 10));
        assert_eq!(q.status, DocumentStatus::Draft);
        assert_eq!(q.sorts.len(), 1);
        assert_eq!(q.sorts[0].field, "title");
        assert_eq!(q.sorts[0].direction, SortDirection::Ascending);

        let filter_str = format!("{:?}", q.filter);
        assert!(filter_str.contains("Equals"));
        assert!(filter_str.contains("title"));
        assert!(filter_str.contains("Contains"));
        assert!(filter_str.contains("description.en"));

        let pop_filters = q.populate_filters.unwrap();
        let cat_attr = AttributeId::try_new("category").unwrap();
        let cat_filter = pop_filters.get(&cat_attr).unwrap();
        let cat_filter_str = format!("{:?}", cat_filter);
        assert!(cat_filter_str.contains("Equals"));
        assert!(cat_filter_str.contains("slug"));
        assert!(cat_filter_str.contains("italian"));
    }
}
