use luminair_common::{
    CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, LOCALE_FIELD_NAME, PUBLISHED_FIELD_NAME, RELATION_ID_FIELD_NAME, UPDATED_FIELD_NAME, domain::{Documents, attributes::AttributeType, persisted::{PersistedDocument, PersistedRelation}}
};

use crate::domain::tables::{Column, ColumnType, ForeignKeyConstraint, Index, Table};

pub mod migration;
pub mod persistence;
pub mod tables;

struct DocumentTables {
    pub main_table: Table,
    pub localization_table: Option<Table>,
    pub relation_tables: Vec<Table>,
}

impl DocumentTables {
    fn new(document: &PersistedDocument, documents: &dyn Documents) -> Self {
        let mut main_table_builder = MainTableBuilder::new(document);
        let mut localization_table_builder = LocalizationTableBuilder::new(document);
        let mut relation_tables_builder = RelationTablesBuilder::new(document);

        handle_document_fields(
            document,
            &mut main_table_builder,
            &mut localization_table_builder,
        );

        for (_, relation) in document.relations.iter() {
            if relation.relation_type.is_owning() {
                relation_tables_builder.push(relation, documents);
            }
        }

        let main_table = main_table_builder.into();
        let localization_table = if document.document_ref.has_localization() {
            Some(localization_table_builder.into())
        } else {
            None
        };
        let relation_tables = relation_tables_builder.into();

        Self {
            main_table,
            localization_table,
            relation_tables,
        }
    }
}

struct MainTableBuilder {
    table_name: String,
    has_draft_and_publish: bool,
    columns: Vec<Column>,
}

struct LocalizationTableBuilder {
    main_table_name: String,
    localization_table_name: String,
    columns: Vec<Column>,
}

struct RelationTablesBuilder {
    main_table_name: String,
    owning_column_name: String,
    relation_tables: Vec<Table>,
}

impl MainTableBuilder {
    fn new(document: &PersistedDocument) -> Self {
        let table_name = document.details.main_table_name.clone();
        let has_draft_and_publish = document.document_ref.has_draft_and_publish();
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

impl LocalizationTableBuilder {
    fn new(document: &PersistedDocument) -> Self {
        let details = &document.details;
        let main_table_name = details.main_table_name.clone();
        let localization_table_name = details.localization_table_name.clone();
        let columns = vec![
            Column::primary_key(DOCUMENT_ID_FIELD_NAME, ColumnType::Integer, None),
            Column::primary_key(LOCALE_FIELD_NAME, ColumnType::Varchar, Some(2)),
        ];
        Self {
            main_table_name,
            localization_table_name,
            columns,
        }
    }

    fn push(&mut self, column: Column) {
        self.columns.push(column);
    }

    fn into(self) -> Table {
        let fkey_constraint = ForeignKeyConstraint::new(
            &self.localization_table_name as &str,
            DOCUMENT_ID_FIELD_NAME,
            &self.main_table_name,
            DOCUMENT_ID_FIELD_NAME,
        );
        let fkey_index = Index::new(
            &self.localization_table_name as &str,
            vec![DOCUMENT_ID_FIELD_NAME],
            false,
        );

        Table::new(
            self.localization_table_name,
            self.columns,
            vec![fkey_constraint],
            vec![fkey_index],
        )
    }
}

impl RelationTablesBuilder {
    fn new(document: &PersistedDocument) -> Self {
        let details = &document.details;
        let main_table_name = details.main_table_name.clone();
        let owning_column_name = details.relation_column_name.clone();
        let relation_tables = Vec::new();
        
        Self {
            main_table_name,
            owning_column_name,
            relation_tables,
        }
    }

    fn push(&mut self, relation: &PersistedRelation, documents: &dyn Documents) {
        let target_document = documents.get_persisted_document_by_ref(relation.target).unwrap();
        let relation_table_name = relation.relation_table_name.clone();
        let inverse_column_name = format!("{}_id", target_document.document_ref.info.singular_name.normalized());

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
                &inverse_column_name as &str,
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
                &inverse_column_name,
                target_document.document_ref.id.normalized().as_ref(),
                DOCUMENT_ID_FIELD_NAME,
            ),
        ];

        let indexes = vec![
            Index::new(&relation_table_name, vec![&self.owning_column_name], false),
            Index::new(&relation_table_name, vec![&inverse_column_name], false),
        ];

        let table = Table::new(relation_table_name, columns, foreign_keys, indexes);
        self.relation_tables.push(table)
    }

    fn into(self) -> Vec<Table> {
        self.relation_tables
    }
}

fn handle_document_fields(
    document: &PersistedDocument,
    main_table_builder: &mut MainTableBuilder,
    localization_table_builder: &mut LocalizationTableBuilder,
) {
    for (_, persisted) in document.fields.iter() {
        let column_type = match persisted.attribute_type {
            AttributeType::Uid => ColumnType::Text,
            AttributeType::Uuid => ColumnType::Uuid,
            AttributeType::Text => ColumnType::Text,
            AttributeType::Integer => ColumnType::Integer,
            AttributeType::Decimal => ColumnType::Decimal,
            AttributeType::Date => ColumnType::Date,
            AttributeType::DateTime => ColumnType::TimestampTZ,
            AttributeType::Boolean => ColumnType::Boolean,
        };

        let column = Column::new(
            persisted.table_column_name.clone(),
            column_type,
            None,
            persisted.required,
            persisted.unique,
            None,
        );

        if persisted.localized {
            localization_table_builder.push(column);
        } else {
            main_table_builder.push(column);
        }
    }
}
