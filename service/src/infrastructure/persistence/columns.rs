use std::borrow::Cow;

use luminair_common::{CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, ID_FIELD_NAME, PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME};

use crate::infrastructure::persistence::query::Column;

/// Common columns

pub const ID_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(ID_FIELD_NAME),
};
pub const DOCUMENT_ID_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(DOCUMENT_ID_FIELD_NAME),
};

pub const CREATED_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(CREATED_FIELD_NAME),
};
pub const UPDATED_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(UPDATED_FIELD_NAME),
};
pub const PUBLISHED_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(PUBLISHED_FIELD_NAME),
};

pub const CREATED_BY_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(CREATED_BY_FIELD_NAME),
};
pub const UPDATED_BY_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(UPDATED_BY_FIELD_NAME),
};
pub const PUBLISHED_BY_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(PUBLISHED_BY_FIELD_NAME),
};

pub const VERSION_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(VERSION_FIELD_NAME),
};
pub const REVISION_COLUMN: Column<'static> = Column {
    qualifier: "m",
    name: Cow::Borrowed(REVISION_FIELD_NAME),
};
