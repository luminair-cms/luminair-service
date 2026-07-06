use crate::domain::query::DocumentStatus;
use luminair_common::{
    CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType,
    PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, SNAPSHOT_ID_FIELD_NAME,
    UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME,
};
use sea_query::ColumnRef;

pub mod find;
pub mod relations;
pub mod write;

const STANDARD_SELECT_COLUMNS: [(&str, &str); 8] = [
    ("m", DOCUMENT_ID_FIELD_NAME),
    ("m", CREATED_FIELD_NAME),
    ("m", UPDATED_FIELD_NAME),
    ("m", CREATED_BY_FIELD_NAME),
    ("m", UPDATED_BY_FIELD_NAME),
    ("m", PUBLISHED_FIELD_NAME),
    ("m", PUBLISHED_BY_FIELD_NAME),
    ("m", REVISION_FIELD_NAME),
];

pub(crate) fn main_select_columns(
    document: &DocumentType,
    status: DocumentStatus,
) -> Vec<ColumnRef> {
    let mut columns: Vec<ColumnRef> = STANDARD_SELECT_COLUMNS
        .iter()
        .map(|c| (*c).into())
        .collect();

    if status == DocumentStatus::Published && document.has_draft_and_publish() {
        columns.push(("m", SNAPSHOT_ID_FIELD_NAME).into());
    }

    for field in &document.fields {
        columns.push(("m", field.id.normalized()).into());
    }

    columns
}
