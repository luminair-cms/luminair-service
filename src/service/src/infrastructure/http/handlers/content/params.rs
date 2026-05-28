use std::collections::HashSet;
use std::str::FromStr;

use luminair_common::{AttributeId, DocumentType, DocumentTypeApiId};

use crate::application::AppState;
use crate::domain::query::DocumentStatus;
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
