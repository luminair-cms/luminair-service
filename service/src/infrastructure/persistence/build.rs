use std::borrow::Cow;

use luminair_common::{AttributeId, DocumentType, OWNING_ID_FIELD_NAME, persistence::QualifiedTable};

use crate::{
    domain::document::DatabaseRowId,
    infrastructure::persistence::{
        columns::{
            CREATED_BY_COLUMN, CREATED_COLUMN, DOCUMENT_ID_COLUMN, ID_COLUMN, INVERSE_ID_COLUMN, OWNING_ID_COLUMN, PUBLISHED_BY_COLUMN, PUBLISHED_COLUMN, REVISION_COLUMN, UPDATED_BY_COLUMN, UPDATED_COLUMN, VERSION_COLUMN
        },
        query::{
            Column, ColumnRef, Condition, Join, JoinType, OrderBy, QueryBuilder,
            SortDirection,
        },
    },
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
