use axum::{extract::FromRequestParts, http::request::Parts, response::Response};
use serde_json::{Map, Value};
use url::form_urlencoded;

#[derive(Debug, Clone, Default)]
pub struct QueryMap(pub Map<String, Value>);

impl<S> FromRequestParts<S> for QueryMap
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();
        Ok(QueryMap(parse_query_to_json(query)))
    }
}

/// Parse a URL-encoded bracket query string into a nested JSON `Map`.
///
/// Supports:
/// - Simple keys: `foo=bar` → `{"foo": "bar"}`
/// - Nested bracket notation: `a[b][c]=v` → `{"a": {"b": {"c": "v"}}}`
/// - Array push notation: `a[]=1&a[]=2` → `{"a": ["1", "2"]}`
///
/// This function is infallible — malformed input is silently discarded rather
/// than propagated as an error, because individual key-parse failures should
/// not abort the entire query.
pub fn parse_query_to_json(query_str: &str) -> Map<String, Value> {
    let mut root = Map::new();

    for (raw_key, raw_val) in form_urlencoded::parse(query_str.as_bytes()) {
        let mut parts = Vec::new();
        let base_end = raw_key.find('[').unwrap_or(raw_key.len());
        parts.push(&raw_key[0..base_end]);

        let mut is_array_push = false;
        let mut rest = &raw_key[base_end..];
        while let Some(start) = rest.find('[') {
            if let Some(end) = rest[start..].find(']') {
                let segment = &rest[start + 1..start + end];
                if segment.is_empty() {
                    is_array_push = true;
                } else {
                    parts.push(segment);
                }
                rest = &rest[start + end + 1..];
            } else {
                break;
            }
        }

        let mut current_map = &mut root;
        for i in 0..parts.len() {
            let part = parts[i].to_string();

            if i == parts.len() - 1 {
                if is_array_push {
                    let entry = current_map
                        .entry(part)
                        .or_insert_with(|| Value::Array(Vec::new()));
                    if let Value::Array(arr) = entry {
                        arr.push(Value::String(raw_val.to_string()));
                    }
                } else {
                    if let Some(existing) = current_map.get_mut(&part) {
                        match existing {
                            Value::Array(arr) => {
                                arr.push(Value::String(raw_val.to_string()));
                            }
                            other => {
                                let old_val = other.clone();
                                *other =
                                    Value::Array(vec![old_val, Value::String(raw_val.to_string())]);
                            }
                        }
                    } else {
                        current_map.insert(part, Value::String(raw_val.to_string()));
                    }
                }
            } else {
                if !current_map.contains_key(&part) {
                    current_map.insert(part.clone(), Value::Object(Map::new()));
                }
                let entry = current_map.get_mut(&part).unwrap();
                if !entry.is_object() {
                    *entry = Value::Object(Map::new());
                }
                current_map = entry.as_object_mut().unwrap();
            }
        }
    }

    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_to_json_nested() {
        let query = "filters[title][$eq]=hello&filters[description][en][$contains]=world&filters[tags][$in][]=rust&filters[tags][$in][]=axum";
        let parsed = parse_query_to_json(query);

        let filters = parsed.get("filters").unwrap().as_object().unwrap();

        let title = filters.get("title").unwrap().as_object().unwrap();
        assert_eq!(title.get("$eq").unwrap().as_str().unwrap(), "hello");

        let description = filters.get("description").unwrap().as_object().unwrap();
        let en = description.get("en").unwrap().as_object().unwrap();
        assert_eq!(en.get("$contains").unwrap().as_str().unwrap(), "world");

        let tags = filters.get("tags").unwrap().as_object().unwrap();
        let r#in = tags.get("$in").unwrap().as_array().unwrap();
        assert_eq!(r#in[0].as_str().unwrap(), "rust");
        assert_eq!(r#in[1].as_str().unwrap(), "axum");
    }
}
