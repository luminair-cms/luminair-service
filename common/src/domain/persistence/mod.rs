use crate::domain::attributes::{Attribute, AttributeBody, AttributeType, RelationTarget};
use crate::domain::documents::Document;
use crate::domain::persistence::tables::{Column, ColumnType, ForeignKeyConstraint, Index, Table};

use std::ops::Deref;
use std::sync::RwLock;

pub mod tables;


/// Represents a Persistence structure on Document in Database
#[derive(Debug)]
pub struct DocumentPersistence {
    pub main_table: Table,
    pub localization_table: Option<Table>,
    pub relation_tables: Vec<Table>,
    pub document_ref: &'static Document
}

impl From<&'static Document> for DocumentPersistence {
    fn from(value: &'static Document) -> Self {
        let mut main_table_builder = MainTableBuilder::new(value);
        let mut localization_table_builder =
            LocalizationTableBuilder::new(main_table_builder.table_name.clone());
        let mut relation_tables_builder =
            RelationTablesBuilder::new(value, main_table_builder.table_name.clone());

        let has_localization = value.has_localization();

        for attribute in value.attributes.iter() {
            handle_document_attribute(
                attribute,
                &mut main_table_builder,
                &mut localization_table_builder,
                &mut relation_tables_builder,
            );
        }

        let main_table = main_table_builder.into();

        let localization_table = if has_localization {
            Some(localization_table_builder.into())
        } else {
            None
        };

        let relation_tables = relation_tables_builder.into();

        Self {
            main_table,
            localization_table,
            relation_tables,
            document_ref: value
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
    fn new(document: &Document) -> Self {
        let table_name = document.id.normalized();
        let has_draft_and_publish = document.has_draft_and_publish();
        let columns = vec![Column::primary_key("document_id", ColumnType::Serial, None)];
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
            "created_at",
            None,
            ColumnType::TimestampTZ,
            None,
            true,
            false,
            Some("now()"),
        ));
        self.columns.push(Column::new(
            "updated_at",
            None,
            ColumnType::TimestampTZ,
            None,
            false,
            false,
            None,
        ));
        if self.has_draft_and_publish {
            self.columns.push(Column::new(
                "published_at",
                None,
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
    fn new(main_table_name: String) -> Self {
        let localization_table_name = format!("{}_localization", main_table_name);
        let columns = vec![
            Column::primary_key("document_id", ColumnType::Integer, None),
            Column::primary_key("locale", ColumnType::Varchar, Some(2)),
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
            "document_id",
            &self.main_table_name,
            "document_id",
        );
        let fkey_index = Index::new(
            &self.localization_table_name as &str,
            vec!["document_id"],
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
    fn new(document: &Document, main_table_name: String) -> Self {
        let owning_column_name = format!("{}_id", document.info.singular_name.normalized());
        let relation_tables = Vec::new();
        Self {
            main_table_name,
            owning_column_name,
            relation_tables,
        }
    }

    fn push(&mut self, id: &str, target: &RwLock<RelationTarget>, ordering: bool) {
        let target_document_lock = target.read().unwrap();

        let target_document = match target_document_lock.deref() {
            RelationTarget::Ref(d) => d,
            _ => panic!(
                "Relation target must be a reference to a document, got {:?}",
                target.read().unwrap()
            ),
        };

        let relation_table_name = format!("{}_{}_relation", &self.main_table_name, id);

        let inverse_column_name = format!("{}_id", target_document.info.singular_name.normalized());

        let mut columns = vec![
            Column::primary_key("relation_id", ColumnType::Serial, None),
            Column::new(
                &self.owning_column_name as &str,
                None,
                ColumnType::Integer,
                None,
                true,
                false,
                None,
            ),
            Column::new(
                &inverse_column_name as &str,
                None,
                ColumnType::Integer,
                None,
                true,
                false,
                None,
            ),
        ];
        if ordering {
            let ordering_column_name = format!("{}_order", inverse_column_name);
            columns.push(Column::new(
                &ordering_column_name as &str,
                None,
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
                "document_id",
            ),
            ForeignKeyConstraint::new(
                &relation_table_name as &str,
                &inverse_column_name,
                target_document.id.normalized().as_ref(),
                "document_id",
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

fn handle_document_attribute(
    attribute: &Attribute,
    main_table_builder: &mut MainTableBuilder,
    localization_table_builder: &mut LocalizationTableBuilder,
    relation_tables_builder: &mut RelationTablesBuilder,
) {
    let id = &attribute.id.normalized() as &str;
    match &attribute.body {
        AttributeBody::Field {
            attribute_type,
            required,
            unique,
            localized,
            ..
        } => {
            let column_type = match attribute_type {
                AttributeType::Uid => ColumnType::Text,
                AttributeType::Uuid => ColumnType::Uuid,
                AttributeType::Text => ColumnType::Text,
                AttributeType::Integer => ColumnType::Integer,
                AttributeType::Decimal => ColumnType::Decimal,
                AttributeType::Date => ColumnType::Date,
                AttributeType::DateTime => ColumnType::TimestampTZ,
                AttributeType::Boolean => ColumnType::Boolean,
            };

            let column = Column::new(id, Some(attribute.id.as_ref()), column_type, None, *required, *unique, None);

            if *localized {
                localization_table_builder.push(column);
            } else {
                main_table_builder.push(column);
            }
        }
        AttributeBody::Relation {
            relation_type,
            target,
            ordering,
        } => {
            if relation_type.is_owning() {
                relation_tables_builder.push(id, target, *ordering);
            }
        }
    };
}
