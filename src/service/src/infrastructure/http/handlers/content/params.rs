use std::collections::HashSet;
use std::str::FromStr;
use serde::Deserialize;
use luminair_common::{AttributeId, DocumentType, DocumentTypeApiId};

use crate::application::AppState;
use crate::domain::query::DocumentStatus;
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
