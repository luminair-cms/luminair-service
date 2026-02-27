use std::borrow::Cow;

use luminair_common::{persistence::QualifiedTable, AttributeId, DocumentType, CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, ID_FIELD_NAME, PUBLISHED_BY_FIELD_NAME, PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME};
use luminair_common::persistence::TableNameProvider;
use crate::domain::sql::modify::CreateStatement;
use crate::domain::sql::query::{Column, ColumnRef, Join, JoinType, OrderBy, QueryBuilder, SortDirection};
use crate::infrastructure::persistence::columns::{
    CREATED_BY_COLUMN, CREATED_COLUMN, DOCUMENT_ID_COLUMN, ID_COLUMN, INVERSE_ID_COLUMN,
    OWNING_ID_COLUMN, PUBLISHED_BY_COLUMN, PUBLISHED_COLUMN, REVISION_COLUMN,
    UPDATED_BY_COLUMN, UPDATED_COLUMN, VERSION_COLUMN,
};

pub fn main_query_builder(schema: &DocumentType) -> QueryBuilder<'_> {
    let table = QualifiedTable::from(schema);
    let columns = main_columns(schema);
    QueryBuilder::from(table).select(columns)
}

/// SELECT r.owning_id, m.id, m.document_id, ...
/// FROM relation_table r
/// JOIN related_table m ON m.id = r.inverse_id
/// WHERE r.owning_id = ANY($1)
pub fn related_query_builder<'a>(
    main_schema: &'a DocumentType,
    related_schema: &'a DocumentType,
    relation_attr: &'a AttributeId,
) -> QueryBuilder<'a> {
    let related_table = QualifiedTable::from(related_schema);
    let relation_table = QualifiedTable::from((main_schema, relation_attr));

    let mut columns = main_columns(related_schema);

    columns.push(Cow::Borrowed(&OWNING_ID_COLUMN));

    QueryBuilder::from(relation_table)
        .join(Join {
            join_type: JoinType::Inner,
            target_table: related_table,
            main_column: Cow::Borrowed(&ID_COLUMN),
            target_column: Cow::Borrowed(&INVERSE_ID_COLUMN),
        })
        .select(columns)
        .order_by(OrderBy {
            column: Cow::Borrowed(&OWNING_ID_COLUMN),
            direction: SortDirection::Ascending,
        })
}

fn main_columns(schema: &DocumentType) -> Vec<ColumnRef<'_>> {
    let mut columns: Vec<ColumnRef<'_>> = vec![
        Cow::Borrowed(&ID_COLUMN),
        Cow::Borrowed(&DOCUMENT_ID_COLUMN),
        Cow::Borrowed(&CREATED_COLUMN),
        Cow::Borrowed(&UPDATED_COLUMN),
        Cow::Borrowed(&CREATED_BY_COLUMN),
        Cow::Borrowed(&UPDATED_BY_COLUMN),
        Cow::Borrowed(&VERSION_COLUMN),
    ];

    if schema.has_draft_and_publish() {
        columns.push(Cow::Borrowed(&PUBLISHED_COLUMN));
        columns.push(Cow::Borrowed(&PUBLISHED_BY_COLUMN));
        columns.push(Cow::Borrowed(&REVISION_COLUMN));
    }

    for id in schema.fields.keys() {
        let column = Column {
            qualifier: "m",
            name: Cow::Owned(id.normalized()),
        };
        columns.push(Cow::Owned(column));
    }

    columns
}

pub fn build_create_statement(document: &DocumentType) -> CreateStatement {
    let table = TableNameProvider::MainTable { document };

    let mut columns = vec![
        Cow::Borrowed(DOCUMENT_ID_FIELD_NAME),
        Cow::Borrowed(CREATED_FIELD_NAME),
        Cow::Borrowed(UPDATED_FIELD_NAME),
        Cow::Borrowed(CREATED_BY_FIELD_NAME),
        Cow::Borrowed(UPDATED_BY_FIELD_NAME),
        Cow::Borrowed(VERSION_FIELD_NAME),
    ];

    if document.has_draft_and_publish() {
        columns.push(Cow::Borrowed(PUBLISHED_FIELD_NAME));
        columns.push(Cow::Borrowed(PUBLISHED_BY_FIELD_NAME));
        columns.push(Cow::Borrowed(REVISION_FIELD_NAME));
    }

    for id in document.fields.keys() {
        columns.push(Cow::Owned(id.normalized()));
    }

    CreateStatement::new(table, columns ).returning(Cow::Borrowed(ID_FIELD_NAME))
}