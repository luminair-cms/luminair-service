use crate::domain::tables::{Column, ColumnType, ForeignKeyConstraint, Index, Table};
use luminair_common::documents::documents::Document;
use luminair_common::{
    documents::Documents, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, PUBLISHED_FIELD_NAME,
    RELATION_ID_FIELD_NAME,
    UPDATED_FIELD_NAME,
};
use luminair_common::documents::attributes::AttributeType;
use luminair_common::documents::attributes::DocumentRelation;

pub mod migration;
pub mod persistence;
pub mod tables;

struct DocumentTables {
    pub main_table: Table,
    pub relation_tables: Vec<Table>,
}

impl DocumentTables {
    fn new(document: &Document, documents: &dyn Documents) -> Self {
        let mut main_table_builder = MainTableBuilder::new(document);
        let mut relation_tables_builder = RelationTablesBuilder::new(document);

        handle_document_fields(document, &mut main_table_builder);

        for (_, relation) in document.relations.iter() {
            if relation.relation_type.is_owning() {
                relation_tables_builder.push(relation, documents);
            }
        }

        let main_table = main_table_builder.into();
        let relation_tables = relation_tables_builder.into();

        Self {
            main_table,
            relation_tables,
        }
    }
}

struct MainTableBuilder {
    table_name: String,
    has_draft_and_publish: bool,
    columns: Vec<Column>,
}

struct RelationTablesBuilder {
    main_table_name: String,
    owning_column_name: String,
    relation_tables: Vec<Table>,
}

impl MainTableBuilder {
    fn new(document: &Document) -> Self {
        let table_name = document.persistence.main_table_name.clone();
        let has_draft_and_publish = document.has_draft_and_publish();
        let columns = vec![Column::primary_key(
            DOCUMENT_ID_FIELD_NAME,
            ColumnType::Serial,
            None,
        )];
        Self {
            table_name,
            has_draft_and_publish,
            columns,
        }
    }

    fn push(&mut self, column: Column) {
        self.columns.push(column);
    }

    fn into(mut self) -> Table {
        self.columns.push(Column::new(
            CREATED_FIELD_NAME,
            ColumnType::TimestampTZ,
            None,
            true,
            false,
            Some("now()"),
        ));
        self.columns.push(Column::new(
            UPDATED_FIELD_NAME,
            ColumnType::TimestampTZ,
            None,
            false,
            false,
            None,
        ));
        if self.has_draft_and_publish {
            self.columns.push(Column::new(
                PUBLISHED_FIELD_NAME,
                ColumnType::TimestampTZ,
                None,
                false,
                false,
                None,
            ));
        }

        // TODO: add created_by_id, updated_by_id columns

        Table::new(self.table_name, self.columns, Vec::new(), Vec::new())
    }
}

impl RelationTablesBuilder {
    fn new(document: &Document) -> Self {
        let document_persistence = &document.persistence;
        let main_table_name = document_persistence.main_table_name.clone();
        let owning_column_name = document_persistence.relation_column_name.clone();
        let relation_tables = Vec::new();

        Self {
            main_table_name,
            owning_column_name,
            relation_tables,
        }
    }

    fn push(&mut self, relation: &DocumentRelation, documents: &dyn Documents) {
        let target_document = documents
            .get_document(&relation.target)
            .unwrap();
        let relation_table_name = relation.relation_table_name.clone();
        let inverse_column_name = &target_document.persistence.relation_column_name as &str;

        let mut columns = vec![
            Column::primary_key(RELATION_ID_FIELD_NAME, ColumnType::Serial, None),
            Column::new(
                &self.owning_column_name as &str,
                ColumnType::Integer,
                None,
                true,
                false,
                None,
            ),
            Column::new(
                inverse_column_name,
                ColumnType::Integer,
                None,
                true,
                false,
                None,
            ),
        ];
        if relation.ordering {
            let ordering_column_name = format!("{}_order", inverse_column_name);
            columns.push(Column::new(
                &ordering_column_name as &str,
                ColumnType::Integer,
                None,
                true,
                false,
                None,
            ));
        }

        let foreign_keys = vec![
            ForeignKeyConstraint::new(
                &relation_table_name as &str,
                &self.owning_column_name,
                &self.main_table_name,
                DOCUMENT_ID_FIELD_NAME,
            ),
            ForeignKeyConstraint::new(
                &relation_table_name as &str,
                inverse_column_name,
                &target_document.persistence.main_table_name,
                DOCUMENT_ID_FIELD_NAME,
            ),
        ];

        let indexes = vec![
            Index::new(
                &relation_table_name as &str,
                vec![&self.owning_column_name as &str],
                false,
            ),
            Index::new(
                &relation_table_name as &str,
                vec![inverse_column_name],
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

fn handle_document_fields(document: &Document, main_table_builder: &mut MainTableBuilder) {
    for (_, persisted) in document.fields.iter() {
        let column_type = if persisted.localized {
            ColumnType::JsonB
        } else {
            match persisted.attribute_type {
                AttributeType::Uid => ColumnType::Text,
                AttributeType::Uuid => ColumnType::Uuid,
                AttributeType::Text => ColumnType::Text,
                AttributeType::Integer => ColumnType::Integer,
                AttributeType::Decimal => ColumnType::Decimal,
                AttributeType::Date => ColumnType::Date,
                AttributeType::DateTime => ColumnType::TimestampTZ,
                AttributeType::Boolean => ColumnType::Boolean,
            }
        };

        let column = Column::new(
            persisted.table_column_name.clone(),
            column_type,
            None,
            persisted.required,
            persisted.unique,
            None,
        );

        main_table_builder.push(column);
    }
}
