use std::collections::{HashSet, HashMap};
use std::str::FromStr;
use serde_json::Value;
use luminair_common::{AttributeId, DocumentType, DocumentTypeApiId, entities::FieldType};

use crate::application::AppState;
use crate::domain::query::{DocumentStatus, FilterExpression, Sort, SortDirection};
use crate::domain::document::content::DomainValue;
use crate::infrastructure::http::api::ApiError;

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

use luminair_common::DocumentTypesRegistry;

pub fn parse_query(
    query_map: &serde_json::Map<String, Value>,
    document_type: &DocumentType,
    registry: &dyn DocumentTypesRegistry,
) -> Result<(
    Option<Vec<AttributeId>>, // populate
    (u16, u16),              // pagination (page, pageSize)
    DocumentStatus,          // status
    FilterExpression,        // main filter
    Option<HashMap<AttributeId, FilterExpression>>, // populate filters
    Vec<Sort>,               // sorts
), ApiError> {
    // 1. Extract populate
    let populate = match query_map.get("populate") {
        Some(Value::String(s)) => {
            let mut set = HashSet::new();
            set.insert(s.clone());
            parse_populate(Some(set), document_type)?
        }
        Some(Value::Array(arr)) => {
            let mut set = HashSet::new();
            for val in arr {
                if let Some(s) = val.as_str() {
                    set.insert(s.to_string());
                }
            }
            parse_populate(Some(set), document_type)?
        }
        _ => None,
    };

    // 2. Extract pagination
    let (page, page_size) = if let Some(Value::Object(pag_map)) = query_map.get("pagination") {
        let page = pag_map
            .get("page")
            .and_then(|v| {
                v.as_str().and_then(|s| s.parse::<u16>().ok())
                    .or_else(|| v.as_u64().map(|n| n as u16))
            })
            .unwrap_or(1);
        let page_size = pag_map
            .get("pageSize")
            .and_then(|v| {
                v.as_str().and_then(|s| s.parse::<u16>().ok())
                    .or_else(|| v.as_u64().map(|n| n as u16))
            })
            .unwrap_or(25);
        (page, page_size)
    } else {
        (1, 25)
    };

    // 3. Extract status
    let status_str = query_map
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("published");
    let status = parse_status(status_str)?;

    // 4. Extract sorts
    let mut sorts = Vec::new();
    if let Some(sort_val) = query_map.get("sort").and_then(|v| v.as_str()) {
        for item in sort_val.split(',') {
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
    }

    // 5. Extract filters recursively
    let mut main_filter = FilterExpression::None;
    let mut populate_filters = HashMap::new();

    if let Some(filters_val) = query_map.get("filters") {
        parse_filters_recursive(
            filters_val,
            "",
            document_type,
            registry,
            &mut main_filter,
            &mut populate_filters,
            None,
        )?;
    }

    let pop_filters = if populate_filters.is_empty() {
        None
    } else {
        Some(populate_filters)
    };

    Ok((populate, (page, page_size), status, main_filter, pop_filters, sorts))
}

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
                    // Operator leaf node
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
                } else if let Some(rel) = document_type.relations.iter().find(|r| r.id.as_ref() == key) {
                    // Relation node - switch document type context to target relation type
                    let target_type = registry
                        .get(&rel.target)
                        .ok_or(ApiError::NotFound)?;

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
                    // Standard field or locale node
                    let is_localized = if let Some(field) = document_type.fields.iter().find(|f| f.id.as_ref() == current_path) {
                        field.field_type == FieldType::LocalizedText
                    } else {
                        false
                    };

                    let new_path = if is_localized && !key.starts_with('$') {
                        format!("{}.{}", current_path, key)
                    } else if current_path.is_empty() {
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
            // Leaf node with no operator: default to $eq
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
            _ => return Err(ApiError::UnprocessableEntity(format!("Expected array or string for operator {}", operator))),
        }

        if operator == "$in" {
            Ok(FilterExpression::In { field: field_path, values: domain_values })
        } else {
            Ok(FilterExpression::NotIn { field: field_path, values: domain_values })
        }
    } else {
        let val_str = match value {
            Value::String(s) => s.as_str(),
            Value::Number(n) => return parse_filter_value(&n.to_string(), field_type).map(|val| {
                if operator == "$ne" {
                    FilterExpression::NotEquals { field: field_path.clone(), value: val }
                } else if operator == "$gt" {
                    FilterExpression::GreaterThan { field: field_path.clone(), value: val }
                } else if operator == "$gte" {
                    FilterExpression::GreaterThanOrEqual { field: field_path.clone(), value: val }
                } else if operator == "$lt" {
                    FilterExpression::LessThan { field: field_path.clone(), value: val }
                } else if operator == "$lte" {
                    FilterExpression::LessThanOrEqual { field: field_path.clone(), value: val }
                } else {
                    FilterExpression::Equals { field: field_path.clone(), value: val }
                }
            }),
            Value::Bool(b) => if *b { "true" } else { "false" },
            _ => return Err(ApiError::UnprocessableEntity(format!("Expected scalar value for operator {}", operator))),
        };

        build_filter_expr_for_operator(field_path, operator, val_str, field_type)
    }
}

fn accumulate_filter(
    main_filter: &mut FilterExpression,
    relation_filters: &mut HashMap<AttributeId, FilterExpression>,
    current_relation: Option<&AttributeId>,
    expr: FilterExpression,
) {
    if let Some(rel_id) = current_relation {
        let current_filter = relation_filters.entry(rel_id.clone()).or_insert(FilterExpression::None);
        let updated = match std::mem::replace(current_filter, FilterExpression::None) {
            FilterExpression::None => expr,
            existing => FilterExpression::And(Box::new(existing), Box::new(expr)),
        };
        *current_filter = updated;
    } else {
        let updated = match std::mem::replace(main_filter, FilterExpression::None) {
            FilterExpression::None => expr,
            existing => FilterExpression::And(Box::new(existing), Box::new(expr)),
        };
        *main_filter = updated;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::http::querystring::parse_query_to_json;
    use luminair_common::entities::{DocumentKind, DocumentRelation, DocumentTitle, DocumentTypeInfo, DocumentField, RelationType};
    use luminair_common::DocumentTypeId;

    #[derive(Debug)]
    struct MockRegistry {
        types: HashMap<DocumentTypeId, &'static DocumentType>,
    }

    impl DocumentTypesRegistry for MockRegistry {
        fn iterate(&self) -> Box<dyn Iterator<Item = &'static DocumentType> + '_> {
            panic!("unimplemented")
        }
        fn get(&self, id: &DocumentTypeId) -> Option<&'static DocumentType> {
            self.types.get(id).copied()
        }
        fn lookup(&self, _api_id: &DocumentTypeApiId) -> Option<&'static DocumentType> {
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
            fields: HashSet::from([
                DocumentField {
                    id: AttributeId::try_new("slug").unwrap(),
                    field_type: FieldType::Text,
                    constraints: HashSet::new(),
                    required: false,
                    unique: false,
                },
            ]),
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
            relations: HashSet::from([
                DocumentRelation {
                    id: AttributeId::try_new("category").unwrap(),
                    target: DocumentTypeId::try_new("category").unwrap(),
                    relation_type: RelationType::HasOne,
                }
            ]),
        }));

        let mut types = HashMap::new();
        types.insert(dt_category.id.clone(), dt_category);
        types.insert(dt_restaurant.id.clone(), dt_restaurant);

        let registry = MockRegistry { types };

        // Test mixed: simple filter, localized filter, and relation filter
        let query = "filters[title][$eq]=hello&filters[description][en][$contains]=world&filters[category][slug][$eq]=italian&sort=title:asc&status=draft&pagination[page]=2&pagination[pageSize]=10";
        let query_map = parse_query_to_json(query).unwrap();

        let (_populate, (page, page_size), status, filter, populate_filters, sorts) =
            parse_query(&query_map, dt_restaurant, &registry).unwrap();

        assert_eq!(page, 2);
        assert_eq!(page_size, 10);
        assert_eq!(status, DocumentStatus::Draft);
        assert_eq!(sorts.len(), 1);
        assert_eq!(sorts[0].field, "title");
        assert_eq!(sorts[0].direction, SortDirection::Ascending);

        // Verify main filter (AND of title = hello and description.en contains world)
        // Expression has Title and Description en
        let filter_str = format!("{:?}", filter);
        assert!(filter_str.contains("Equals"));
        assert!(filter_str.contains("title"));
        assert!(filter_str.contains("Contains"));
        assert!(filter_str.contains("description.en"));

        // Verify populate relation filter (category relation has slug = italian)
        let pop_filters = populate_filters.unwrap();
        let cat_attr = AttributeId::try_new("category").unwrap();
        let cat_filter = pop_filters.get(&cat_attr).unwrap();
        let cat_filter_str = format!("{:?}", cat_filter);
        assert!(cat_filter_str.contains("Equals"));
        assert!(cat_filter_str.contains("slug"));
        assert!(cat_filter_str.contains("italian"));
    }
}
