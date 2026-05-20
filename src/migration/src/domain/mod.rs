use crate::domain::tables::{Column, ColumnType, ForeignKeyConstraint, Index, Table};

use luminair_common::entities::{DocumentField, IntegerSize};
use luminair_common::{
    CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType,
    DocumentTypesRegistry, ID_FIELD_NAME, OWNING_ID_FIELD_NAME, PUBLISHED_BY_FIELD_NAME,
    PUBLISHED_FIELD_NAME, RELATION_ID_FIELD_NAME, REVISION_FIELD_NAME, STATUS_FIELD_NAME,
    TARGET_DOCUMENT_ID_FIELD_NAME, UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME,
    entities::{DocumentRelation, FieldType},
};

pub mod migration;
pub mod persistence;
pub mod tables;

struct DocumentTables {
    pub identity_table: Table,
    pub collection_table: Table,
    pub relation_tables: Vec<Table>,
}

impl DocumentTables {
    fn new(document: &DocumentType, documents: &dyn DocumentTypesRegistry) -> Self {
        let identity_table_builder = IdentityTableBuilder::new(document);
        let mut main_table_builder = MainTableBuilder::new(document);
        let mut relation_tables_builder = RelationTablesBuilder::new(document);

        handle_document_fields(document, &mut main_table_builder);

        for relation in document.relations.iter() {
            if relation.relation_type.is_owning() {
                relation_tables_builder.push(relation, documents);
            }
        }

        let identity_table = identity_table_builder.into();
        let collection_table = main_table_builder.into();
        let relation_tables = relation_tables_builder.into();

        Self {
            identity_table,
            collection_table,
            relation_tables,
        }
    }
}

struct IdentityTableBuilder {
    table_name: String,
}

impl IdentityTableBuilder {
    fn new(document: &DocumentType) -> Self {
        let table_name = format!("{}_documents", document.id.normalized());
        Self { table_name }
    }

    fn into(self) -> Table {
        let columns = vec![
            Column::primary_key(DOCUMENT_ID_FIELD_NAME, ColumnType::Uuid, None),
            Column::new(STATUS_FIELD_NAME, ColumnType::Text, None, true, false, None),
            Column::new(
                CREATED_FIELD_NAME,
                ColumnType::TimestampTZ,
                None,
                true,
                false,
                Some("now()"),
            ),
            Column::new(
                UPDATED_FIELD_NAME,
                ColumnType::TimestampTZ,
                None,
                false,
                false,
                None,
            ),
            Column::new(
                CREATED_BY_FIELD_NAME,
                ColumnType::Text,
                None,
                false,
                false,
                None,
            ),
            Column::new(
                UPDATED_BY_FIELD_NAME,
                ColumnType::Text,
                None,
                false,
                false,
                None,
            ),
        ];

        Table::new(self.table_name, columns, Vec::new(), Vec::new())
    }
}

struct MainTableBuilder {
    table_name: String,
    identity_table_name: String,
    has_draft_and_publish: bool,
    columns: Vec<Column>,
}

impl MainTableBuilder {
    fn new(document: &DocumentType) -> Self {
        let table_name = document.id.normalized();
        let identity_table_name = format!("{}_documents", table_name);
        let has_draft_and_publish = document.has_draft_and_publish();
        let columns = vec![
            Column::primary_key(ID_FIELD_NAME, ColumnType::Serial, None),
            Column::new(
                DOCUMENT_ID_FIELD_NAME,
                ColumnType::Uuid,
                None,
                true,
                false,
                None,
            ),
        ];
        Self {
            table_name,
            identity_table_name,
            has_draft_and_publish,
            columns,
        }
    }

    fn push(&mut self, column: Column) {
        self.columns.push(column);
    }

    fn into(mut self) -> Table {
        self.columns.extend(vec![
            Column::new(
                CREATED_FIELD_NAME,
                ColumnType::TimestampTZ,
                None,
                true,
                false,
                Some("now()"),
            ),
            Column::new(
                UPDATED_FIELD_NAME,
                ColumnType::TimestampTZ,
                None,
                false,
                false,
                None,
            ),
            Column::new(
                CREATED_BY_FIELD_NAME,
                ColumnType::Text,
                None,
                false,
                false,
                None,
            ),
            Column::new(
                UPDATED_BY_FIELD_NAME,
                ColumnType::Text,
                None,
                false,
                false,
                None,
            ),
            Column::new(
                VERSION_FIELD_NAME,
                ColumnType::Integer(IntegerSize::Int32),
                None,
                true,
                false,
                None,
            ),
        ]);

        if self.has_draft_and_publish {
            self.columns.extend(vec![
                Column::new(
                    PUBLISHED_FIELD_NAME,
                    ColumnType::TimestampTZ,
                    None,
                    false,
                    false,
                    None,
                ),
                Column::new(
                    PUBLISHED_BY_FIELD_NAME,
                    ColumnType::Text,
                    None,
                    false,
                    false,
                    None,
                ),
                Column::new(
                    REVISION_FIELD_NAME,
                    ColumnType::Integer(IntegerSize::Int32),
                    None,
                    true,
                    false,
                    None,
                ),
            ]);
        }

        let foreign_keys = vec![ForeignKeyConstraint::new(
            &self.table_name as &str,
            DOCUMENT_ID_FIELD_NAME,
            &self.identity_table_name,
            DOCUMENT_ID_FIELD_NAME,
        )];

        let mut indexes = vec![Index::new(
            &self.table_name as &str,
            vec![DOCUMENT_ID_FIELD_NAME],
            false,
        )];

        if self.has_draft_and_publish {
            indexes.push(
                Index::new(&self.table_name as &str, vec![DOCUMENT_ID_FIELD_NAME], true)
                    .with_where(format!("{} IS NULL", PUBLISHED_FIELD_NAME)),
            );
            indexes.push(
                Index::new(&self.table_name as &str, vec![DOCUMENT_ID_FIELD_NAME], true)
                    .with_where(format!("{} IS NOT NULL", PUBLISHED_FIELD_NAME)),
            );
        }

        Table::new(self.table_name, self.columns, foreign_keys, indexes)
    }
}

struct RelationTablesBuilder {
    main_table_name: String,
    relation_tables: Vec<Table>,
}

impl RelationTablesBuilder {
    fn new(document: &DocumentType) -> Self {
        let main_table_name = document.id.normalized();
        let relation_tables = Vec::new();

        Self {
            main_table_name,
            relation_tables,
        }
    }

    fn push(&mut self, relation: &DocumentRelation, documents: &dyn DocumentTypesRegistry) {
        let target_document = documents.get(&relation.target).unwrap();
        let target_table_name = target_document.id.normalized();
        let target_identity_table_name = format!("{}_documents", target_table_name);
        let relation_table_name = format!(
            "{}_{}_relation",
            self.main_table_name,
            relation.id.normalized()
        );

        let columns = vec![
            Column::primary_key(RELATION_ID_FIELD_NAME, ColumnType::Serial, None),
            Column::new(
                OWNING_ID_FIELD_NAME,
                ColumnType::Integer(IntegerSize::Int32),
                None,
                true,
                false,
                None,
            ),
            Column::new(
                TARGET_DOCUMENT_ID_FIELD_NAME,
                ColumnType::Uuid,
                None,
                true,
                false,
                None,
            ),
        ];

        let foreign_keys = vec![
            ForeignKeyConstraint::new(
                &relation_table_name as &str,
                OWNING_ID_FIELD_NAME,
                &self.main_table_name,
                ID_FIELD_NAME,
            ),
            ForeignKeyConstraint::new(
                &relation_table_name as &str,
                TARGET_DOCUMENT_ID_FIELD_NAME,
                &target_identity_table_name,
                DOCUMENT_ID_FIELD_NAME,
            ),
        ];

        let indexes = vec![
            Index::new(
                &relation_table_name as &str,
                vec![OWNING_ID_FIELD_NAME],
                false,
            ),
            Index::new(
                &relation_table_name as &str,
                vec![TARGET_DOCUMENT_ID_FIELD_NAME],
                false,
            ),
        ];

        let table = Table::new(relation_table_name, columns, foreign_keys, indexes);
        self.relation_tables.push(table)
    }

    fn into(self) -> Vec<Table> {
        self.relation_tables
    }
}

fn handle_document_fields(document: &DocumentType, main_table_builder: &mut MainTableBuilder) {
    for field in document.fields.iter() {
        let column_type = infer_column_type(field);

        let column = Column::new(
            field.id.normalized(),
            column_type,
            None,
            field.required,
            field.unique,
            None,
        );

        main_table_builder.push(column);
    }
}

fn infer_column_type(field: &DocumentField) -> ColumnType {
    match field.field_type {
        FieldType::Uid => ColumnType::Text,
        FieldType::Uuid => ColumnType::Uuid,
        FieldType::Text => ColumnType::Text,
        FieldType::LocalizedText => ColumnType::JsonB,
        FieldType::Integer(size) => ColumnType::Integer(size),
        FieldType::Decimal { precision, scale } => ColumnType::Decimal { precision, scale },
        FieldType::Date => ColumnType::Date,
        FieldType::DateTime => ColumnType::TimestampTZ,
        FieldType::Boolean => ColumnType::Boolean,
        FieldType::Json => ColumnType::JsonB,
    }
}
