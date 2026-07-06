use std::collections::HashMap;

use luminair_common::{AttributeId, DocumentType, DocumentTypesRegistry, entities::FieldType};
use serde_json::Value;

use crate::domain::document::content::DomainValue;
use crate::domain::query::{DocumentStatus, FilterExpression, Sort, SortDirection};
use crate::infrastructure::http::api::ApiError;

// ─── Constants ────────────────────────────────────────────────────────────────

/// The wildcard token that, when supplied as the single `populate` value,
/// expands to every owning relation declared on the document type.
const POPULATE_WILDCARD: &str = "*";

// ─── Public output types ──────────────────────────────────────────────────────

/// Schema-agnostic representation of every bracket query parameter.
///
/// Produced by [`parse_raw_query`] without any domain knowledge.
/// Use [`parse_query`] to validate and resolve it against a [`DocumentType`].
pub(super) struct RawQueryParams {
    /// `?populate=*` / `?populate[]=field` / `?populate=field`
    pub populate: Option<std::collections::HashSet<String>>,
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
#[derive(Debug)]
pub struct DocumentQuery {
    pub populate: Option<Vec<AttributeId>>,
    pub pagination: (u16, u16),
    pub status: DocumentStatus,
    pub filter: FilterExpression,
    pub populate_filters: Option<HashMap<AttributeId, FilterExpression>>,
    pub sorts: Vec<Sort>,
}

// ─── Phase 0: structural parse (no schema knowledge) ─────────────────────────

/// Parse a nested query-string map into [`RawQueryParams`] with no schema knowledge.
///
/// This is a pure structural transformation: it extracts the well-known top-level
/// keys (`populate`, `pagination`, `status`, `sort`, `filters`) from the already-
/// decoded bracket map and returns them in typed form, without any domain validation.
pub(super) fn parse_raw_query(
    query_map: &serde_json::Map<String, Value>,
    pagination_settings: &crate::application::PaginationSettings,
) -> RawQueryParams {
    use std::collections::HashSet;

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
            .unwrap_or(pagination_settings.default_page_size)
            .min(pagination_settings.max_page_size);
        (page, page_size)
    } else {
        (1, pagination_settings.default_page_size)
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

    // filters — kept opaque for the validation phase
    let filters = query_map.get("filters").cloned();

    RawQueryParams { populate, pagination, status, sorts, filters }
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Parse and validate all query parameters against the given [`DocumentType`] schema.
///
/// Internally calls [`parse_raw_query`] for structural parsing, then validates and
/// resolves each field through the three-phase pipeline:
///
/// 1. **Structural parse** — [`parse_raw_query`] extracts well-known keys with no
///    schema awareness.
/// 2. **Schema validation** — [`validate_filter_tree`] walks the filter JSON and
///    resolves field types and relation contexts.  Unknown fields produce a
///    `422 Unprocessable Entity` error rather than silently falling back to `Text`.
/// 3. **Domain mapping** — [`build_filter_expression`] converts the validated tree
///    into [`FilterExpression`] values using [`DomainValue::parse`] for type
///    coercion, keeping a single canonical string→domain codec.
pub fn parse_query(
    query_map: &serde_json::Map<String, Value>,
    document_type: &DocumentType,
    registry: &dyn DocumentTypesRegistry,
    pagination_settings: &crate::application::PaginationSettings,
) -> Result<DocumentQuery, ApiError> {
    let raw = parse_raw_query(query_map, pagination_settings);

    let status = parse_status(&raw.status)?;
    let populate = resolve_populate(raw.populate, document_type)?;
    let sorts = resolve_sorts(raw.sorts, document_type)?;

    let (filter, populate_filters) = if let Some(filter_value) = raw.filters {
        let validated = validate_filter_tree(&filter_value, "", document_type, registry)?;
        let (main_nodes, rel_map) = split_relation_filters(validated);
        let main_filter = build_filter_expression(main_nodes)?;
        let pop_filters = rel_map
            .into_iter()
            .map(|(attr, nodes)| Ok((attr, build_filter_expression(nodes)?)))
            .collect::<Result<HashMap<_, _>, ApiError>>()?;
        let pop_filters = if pop_filters.is_empty() { None } else { Some(pop_filters) };
        (main_filter, pop_filters)
    } else {
        (FilterExpression::None, None)
    };

    Ok(DocumentQuery {
        populate,
        pagination: raw.pagination,
        status,
        filter,
        populate_filters,
        sorts,
    })
}

// ─── Phase 1: Operator enum ───────────────────────────────────────────────────

/// Recognized filter operators, resolved from their raw string representation.
///
/// Centralizes all alias handling (`$notIn` / `$not_in`, `$startsWith` /
/// `$starts_with`, etc.) in a single place so new aliases require one change only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    NotIn,
    Contains,
    StartsWith,
    EndsWith,
    IsNull,
    IsNotNull,
}

impl FilterOperator {
    fn from_str(s: &str) -> Result<Self, ApiError> {
        match s {
            "$eq" | "" => Ok(Self::Eq),
            "$ne" => Ok(Self::Ne),
            "$gt" => Ok(Self::Gt),
            "$gte" => Ok(Self::Gte),
            "$lt" => Ok(Self::Lt),
            "$lte" => Ok(Self::Lte),
            "$in" => Ok(Self::In),
            "$notIn" | "$not_in" => Ok(Self::NotIn),
            "$contains" => Ok(Self::Contains),
            "$startsWith" | "$starts_with" => Ok(Self::StartsWith),
            "$endsWith" | "$ends_with" => Ok(Self::EndsWith),
            "$null" => Ok(Self::IsNull),
            "$notNull" | "$not_null" => Ok(Self::IsNotNull),
            other => Err(ApiError::UnprocessableEntity(format!(
                "Unsupported filter operator: {}",
                other
            ))),
        }
    }

    /// Whether this operator consumes a list of values (`$in` / `$notIn`).
    fn is_list_operator(self) -> bool {
        matches!(self, Self::In | Self::NotIn)
    }

    /// Whether this operator is a null-check (`$null` / `$notNull`).
    fn is_null_check(self) -> bool {
        matches!(self, Self::IsNull | Self::IsNotNull)
    }
}

// ─── Phase 2: Validated intermediate tree ─────────────────────────────────────

/// A filter tree node that has been validated against the document type schema.
///
/// Produced by [`validate_filter_tree`].  Each node carries resolved field types
/// and raw string values so the domain mapping phase can coerce them without
/// re-consulting the schema.
enum ValidatedFilterNode {
    /// A single scalar comparison: `field op value`.
    Scalar {
        field_path: String,
        operator: FilterOperator,
        field_type: FieldType,
        raw_value: String,
    },
    /// A list comparison: `field $in [v1, v2, ...]`.
    List {
        field_path: String,
        operator: FilterOperator,
        field_type: FieldType,
        raw_values: Vec<String>,
    },
    /// A null-check: `field $null true` / `field $notNull true`.
    NullCheck {
        field_path: String,
        /// `true` → IS NOT NULL, `false` → IS NULL.
        is_not_null: bool,
    },
    /// Sub-filter targeting a relation's own fields.
    Relation {
        relation_id: AttributeId,
        children: Vec<ValidatedFilterNode>,
    },
}

// ─── Phase 2: Schema validation ───────────────────────────────────────────────

/// Walk the filter JSON value and validate it against the document type schema,
/// returning a tree of [`ValidatedFilterNode`]s.
///
/// `current_path` accumulates the dot-separated field path as we recurse into
/// nested objects (e.g. `description` → `description.en`).
///
/// # Errors
///
/// Returns `ApiError::UnprocessableEntity` for:
/// - Unknown field names (no silent fallback to `FieldType::Text`)
/// - Relation keys whose target type is not in the registry
/// - Unrecognized operator strings
fn validate_filter_tree(
    value: &Value,
    current_path: &str,
    document_type: &DocumentType,
    registry: &dyn DocumentTypesRegistry,
) -> Result<Vec<ValidatedFilterNode>, ApiError> {
    let mut nodes = Vec::new();

    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key.starts_with('$') {
                    // Operator leaf — build a node for the current field path.
                    let operator = FilterOperator::from_str(key)?;
                    let node = build_validated_node(current_path, operator, child, document_type)?;
                    nodes.push(node);
                } else if let Some(rel) =
                    document_type.relations.iter().find(|r| r.id.as_ref() == key)
                {
                    // Relation key — recurse with the target document type.
                    let target_type = registry
                        .get(&rel.target)
                        .ok_or(ApiError::NotFound)?;

                    let children = validate_filter_tree(child, "", target_type, registry)?;
                    nodes.push(ValidatedFilterNode::Relation {
                        relation_id: rel.id.clone(),
                        children,
                    });
                } else {
                    // Regular field key or locale segment — extend the path and recurse.
                    let new_path = if current_path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", current_path, key)
                    };
                    let mut child_nodes =
                        validate_filter_tree(child, &new_path, document_type, registry)?;
                    nodes.append(&mut child_nodes);
                }
            }
        }
        // Bare value with no explicit operator — treat as an implicit `$eq`.
        _ => {
            let node = build_validated_node(current_path, FilterOperator::Eq, value, document_type)?;
            nodes.push(node);
        }
    }

    Ok(nodes)
}

/// Resolve a field path to its [`FieldType`] from the document type schema.
///
/// Only the **base** field name (the first segment before any `.`) is looked up,
/// because nested segments are locale codes or JSON sub-keys, not separate fields.
///
/// Returns `Err` if the field does not exist on the document type — no silent
/// fallback.
fn resolve_field_type(
    field_path: &str,
    document_type: &DocumentType,
) -> Result<FieldType, ApiError> {
    let base_field = field_path.split('.').next().unwrap_or(field_path);
    document_type
        .fields
        .iter()
        .find(|f| f.id.as_ref() == base_field)
        .map(|f| f.field_type)
        .ok_or_else(|| {
            ApiError::UnprocessableEntity(format!("Unknown filter field: '{}'", base_field))
        })
}

/// Build a single [`ValidatedFilterNode`] for a `(field_path, operator, json_value)` triple.
fn build_validated_node(
    field_path: &str,
    operator: FilterOperator,
    value: &Value,
    document_type: &DocumentType,
) -> Result<ValidatedFilterNode, ApiError> {
    if operator.is_null_check() {
        // $null / $notNull — value is a boolean controlling polarity.
        let polarity = match value {
            Value::Bool(b) => *b,
            Value::String(s) => s.parse::<bool>().unwrap_or(true),
            _ => true,
        };
        // IsNull operator: polarity=true → IS NULL, polarity=false → IS NOT NULL.
        // IsNotNull operator: polarity=true → IS NOT NULL, polarity=false → IS NULL.
        let is_not_null = match operator {
            FilterOperator::IsNull => !polarity,
            FilterOperator::IsNotNull => polarity,
            _ => unreachable!(),
        };
        return Ok(ValidatedFilterNode::NullCheck {
            field_path: field_path.to_owned(),
            is_not_null,
        });
    }

    let field_type = resolve_field_type(field_path, document_type)?;

    if operator.is_list_operator() {
        // $in / $notIn — value must be an array or a single string.
        let raw_values: Vec<String> = match value {
            Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            Value::String(s) => vec![s.clone()],
            _ => {
                return Err(ApiError::UnprocessableEntity(format!(
                    "Expected an array for operator {:?} on field '{}'",
                    operator, field_path
                )))
            }
        };
        return Ok(ValidatedFilterNode::List {
            field_path: field_path.to_owned(),
            operator,
            field_type,
            raw_values,
        });
    }

    // Scalar operator — normalise the JSON value to a string, then validate.
    let raw_value = json_value_to_raw_string(value).ok_or_else(|| {
        ApiError::UnprocessableEntity(format!(
            "Expected a scalar value for operator {:?} on field '{}'",
            operator, field_path
        ))
    })?;

    Ok(ValidatedFilterNode::Scalar {
        field_path: field_path.to_owned(),
        operator,
        field_type,
        raw_value,
    })
}

/// Convert a scalar JSON value to its raw string representation.
///
/// Returns `None` for arrays, objects, and `null` (non-scalar types).
fn json_value_to_raw_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "true" } else { "false" }.to_owned()),
        _ => None,
    }
}

// ─── Phase 3: Split relation sub-filters ─────────────────────────────────────

/// Separate top-level field filter nodes from per-relation sub-filter nodes.
///
/// Returns `(main_nodes, relation_map)` where `relation_map` keys are relation
/// attribute IDs and values are the children of the corresponding
/// [`ValidatedFilterNode::Relation`] nodes.
fn split_relation_filters(
    nodes: Vec<ValidatedFilterNode>,
) -> (Vec<ValidatedFilterNode>, HashMap<AttributeId, Vec<ValidatedFilterNode>>) {
    let mut main_nodes = Vec::new();
    let mut rel_map: HashMap<AttributeId, Vec<ValidatedFilterNode>> = HashMap::new();

    for node in nodes {
        match node {
            ValidatedFilterNode::Relation { relation_id, children } => {
                rel_map.entry(relation_id).or_default().extend(children);
            }
            other => main_nodes.push(other),
        }
    }

    (main_nodes, rel_map)
}

// ─── Phase 4: Domain mapping ──────────────────────────────────────────────────

/// Convert a list of [`ValidatedFilterNode`]s into a single [`FilterExpression`].
///
/// Multiple nodes are combined with `And`.  Uses [`DomainValue::parse`] for all
/// type coercion — the single canonical `&str → DomainValue` path.
fn build_filter_expression(
    nodes: Vec<ValidatedFilterNode>,
) -> Result<FilterExpression, ApiError> {
    let mut result = FilterExpression::None;

    for node in nodes {
        let expr = node_to_expression(node)?;
        result = match result {
            FilterExpression::None => expr,
            existing => FilterExpression::And(Box::new(existing), Box::new(expr)),
        };
    }

    Ok(result)
}

/// Convert a single [`ValidatedFilterNode`] into a [`FilterExpression`].
fn node_to_expression(node: ValidatedFilterNode) -> Result<FilterExpression, ApiError> {
    match node {
        ValidatedFilterNode::NullCheck { field_path, is_not_null } => {
            if is_not_null {
                Ok(FilterExpression::IsNotNull { field: field_path })
            } else {
                Ok(FilterExpression::IsNull { field: field_path })
            }
        }

        ValidatedFilterNode::List { field_path, operator, field_type, raw_values } => {
            let values = raw_values
                .into_iter()
                .map(|raw| {
                    DomainValue::parse(&raw, field_type)
                        .map_err(|e| ApiError::UnprocessableEntity(e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;

            match operator {
                FilterOperator::In => Ok(FilterExpression::In { field: field_path, values }),
                FilterOperator::NotIn => Ok(FilterExpression::NotIn { field: field_path, values }),
                _ => unreachable!("only In/NotIn reach the List branch"),
            }
        }

        ValidatedFilterNode::Scalar { field_path, operator, field_type, raw_value } => {
            scalar_to_expression(field_path, operator, raw_value, field_type)
        }

        // Relation nodes are split out by split_relation_filters before this function
        // is called; if one reaches here it is a logic error.
        ValidatedFilterNode::Relation { .. } => {
            Err(ApiError::InternalServerError(
                "Relation filter node reached domain mapping phase unexpectedly".to_owned(),
            ))
        }
    }
}

/// Map a scalar `(field, operator, raw_value, field_type)` into a [`FilterExpression`].
///
/// Text-only operators (`$contains`, `$startsWith`, `$endsWith`) bypass
/// [`DomainValue::parse`] and work directly on the raw string, moving it into the expression.
fn scalar_to_expression(
    field: String,
    operator: FilterOperator,
    raw: String,
    field_type: FieldType,
) -> Result<FilterExpression, ApiError> {
    match operator {
        // Text-only operators — no type coercion needed. Move the raw String directly.
        FilterOperator::Contains => Ok(FilterExpression::Contains { field, value: raw }),
        FilterOperator::StartsWith => Ok(FilterExpression::StartsWith { field, value: raw }),
        FilterOperator::EndsWith => Ok(FilterExpression::EndsWith { field, value: raw }),

        // Typed comparison operators — parse via the canonical codec.
        op => {
            let value = DomainValue::parse(&raw, field_type)
                .map_err(|e| ApiError::UnprocessableEntity(e.to_string()))?;

            Ok(match op {
                FilterOperator::Eq => FilterExpression::Equals { field, value },
                FilterOperator::Ne => FilterExpression::NotEquals { field, value },
                FilterOperator::Gt => FilterExpression::GreaterThan { field, value },
                FilterOperator::Gte => FilterExpression::GreaterThanOrEqual { field, value },
                FilterOperator::Lt => FilterExpression::LessThan { field, value },
                FilterOperator::Lte => FilterExpression::LessThanOrEqual { field, value },
                // Already handled above; these arms silence the exhaustiveness check.
                FilterOperator::Contains
                | FilterOperator::StartsWith
                | FilterOperator::EndsWith
                | FilterOperator::In
                | FilterOperator::NotIn
                | FilterOperator::IsNull
                | FilterOperator::IsNotNull => unreachable!(),
            })
        }
    }
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
    fields: Option<std::collections::HashSet<String>>,
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

/// Validate sort field names against the document type schema and build [`Sort`] values.
///
/// Rejects sorts on unknown fields with `422 Unprocessable Entity`.
fn resolve_sorts(
    raw_sorts: Vec<(String, SortDirection)>,
    document_type: &DocumentType,
) -> Result<Vec<Sort>, ApiError> {
    raw_sorts
        .into_iter()
        .map(|(field, direction)| {
            let field_exists = document_type.fields.iter().any(|f| f.id.as_ref() == field);
            if !field_exists {
                return Err(ApiError::UnprocessableEntity(format!(
                    "Unknown sort field: '{}'",
                    field
                )));
            }
            Ok(Sort { field, direction })
        })
        .collect()
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
    use std::collections::{HashMap, HashSet};

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
            &filters[category][slug][$eq]=italian\
            &sort=title:asc\
            &status=draft\
            &pagination[page]=2\
            &pagination[pageSize]=10";
        let query_map = parse_query_to_json(query);

        let q = parse_query(&query_map, dt_restaurant, &registry, &crate::application::PaginationSettings::default()).unwrap();

        assert_eq!(q.pagination, (2, 10));
        assert_eq!(q.status, DocumentStatus::Draft);
        assert_eq!(q.sorts.len(), 1);
        assert_eq!(q.sorts[0].field, "title");
        assert_eq!(q.sorts[0].direction, SortDirection::Ascending);

        let filter_str = format!("{:?}", q.filter);
        assert!(filter_str.contains("Equals"));
        assert!(filter_str.contains("title"));

        let pop_filters = q.populate_filters.unwrap();
        let cat_attr = AttributeId::try_new("category").unwrap();
        let cat_filter = pop_filters.get(&cat_attr).unwrap();
        let cat_filter_str = format!("{:?}", cat_filter);
        assert!(cat_filter_str.contains("Equals"));
        assert!(cat_filter_str.contains("slug"));
        assert!(cat_filter_str.contains("italian"));
    }

    #[test]
    fn test_unknown_filter_field_returns_error() {
        let dt: &'static DocumentType = Box::leak(Box::new(DocumentType {
            id: DocumentTypeId::try_new("article").unwrap(),
            kind: DocumentKind::Collection,
            info: DocumentTypeInfo {
                title: DocumentTitle::try_new("Article").unwrap(),
                singular_name: DocumentTypeId::try_new("article").unwrap(),
                plural_name: DocumentTypeId::try_new("articles").unwrap(),
                description: None,
            },
            options: None,
            fields: HashSet::from([DocumentField {
                id: AttributeId::try_new("title").unwrap(),
                field_type: FieldType::Text,
                constraints: HashSet::new(),
                required: false,
                unique: false,
            }]),
            relations: HashSet::new(),
        }));

        let registry = MockRegistry { types: HashMap::new() };
        let query = "filters[nonexistent][$eq]=foo";
        let query_map = parse_query_to_json(query);

        let result = parse_query(&query_map, dt, &registry, &crate::application::PaginationSettings::default());
        assert!(matches!(result, Err(ApiError::UnprocessableEntity(_))));
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("nonexistent"), "error should name the bad field: {}", msg);
    }

    #[test]
    fn test_unknown_sort_field_returns_error() {
        let dt: &'static DocumentType = Box::leak(Box::new(DocumentType {
            id: DocumentTypeId::try_new("article2").unwrap(),
            kind: DocumentKind::Collection,
            info: DocumentTypeInfo {
                title: DocumentTitle::try_new("Article2").unwrap(),
                singular_name: DocumentTypeId::try_new("article2").unwrap(),
                plural_name: DocumentTypeId::try_new("article2s").unwrap(),
                description: None,
            },
            options: None,
            fields: HashSet::from([DocumentField {
                id: AttributeId::try_new("title").unwrap(),
                field_type: FieldType::Text,
                constraints: HashSet::new(),
                required: false,
                unique: false,
            }]),
            relations: HashSet::new(),
        }));

        let registry = MockRegistry { types: HashMap::new() };
        let query = "sort=ghost_field:asc";
        let query_map = parse_query_to_json(query);

        let result = parse_query(&query_map, dt, &registry, &crate::application::PaginationSettings::default());
        assert!(matches!(result, Err(ApiError::UnprocessableEntity(_))));
    }

    #[test]
    fn test_filter_operator_aliases() {
        assert_eq!(FilterOperator::from_str("$notIn").unwrap(), FilterOperator::NotIn);
        assert_eq!(FilterOperator::from_str("$not_in").unwrap(), FilterOperator::NotIn);
        assert_eq!(FilterOperator::from_str("$startsWith").unwrap(), FilterOperator::StartsWith);
        assert_eq!(FilterOperator::from_str("$starts_with").unwrap(), FilterOperator::StartsWith);
        assert_eq!(FilterOperator::from_str("$endsWith").unwrap(), FilterOperator::EndsWith);
        assert_eq!(FilterOperator::from_str("$ends_with").unwrap(), FilterOperator::EndsWith);
        assert_eq!(FilterOperator::from_str("$notNull").unwrap(), FilterOperator::IsNotNull);
        assert_eq!(FilterOperator::from_str("$not_null").unwrap(), FilterOperator::IsNotNull);
        assert!(FilterOperator::from_str("$bogus").is_err());
    }
}
