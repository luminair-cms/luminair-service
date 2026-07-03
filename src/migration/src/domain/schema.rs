use crate::domain::tables::{Column, ColumnType, ForeignKeyConstraint, Index, Table};

use luminair_common::entities::{DocumentField, IntegerSize};
use luminair_common::{
    CREATED_BY_FIELD_NAME, CREATED_FIELD_NAME, DOCUMENT_ID_FIELD_NAME, DocumentType,
    DocumentTypesRegistry, OWNING_DOCUMENT_ID_FIELD_NAME, PUBLISHED_BY_FIELD_NAME,
    PUBLISHED_FIELD_NAME, REVISION_FIELD_NAME, STATUS_FIELD_NAME, TARGET_DOCUMENT_ID_FIELD_NAME,
    UPDATED_BY_FIELD_NAME, UPDATED_FIELD_NAME, VERSION_FIELD_NAME,
    entities::{DocumentRelation, FieldType},
};

const SNAPSHOT_ID_FIELD_NAME: &str = "snapshot_id";

pub struct DocumentTables {
    pub tables: Vec<Table>,
}

impl DocumentTables {
    pub fn new(document: &DocumentType, documents: &dyn DocumentTypesRegistry) -> Self {
        let mut tables = Vec::new();

        let mut main_table_builder = MainTableBuilder::new(document);

        if document.has_draft_and_publish() {
            let mut snapshots_table_builder = SnapshotsTableBuilder::new(document);
            handle_document_fields(
                document,
                &mut main_table_builder,
                Some(&mut snapshots_table_builder),
            );
            let main_table = main_table_builder.into();
            let snapshots_table = snapshots_table_builder.into();

            tables.push(main_table);
            tables.push(snapshots_table);
        } else {
            handle_document_fields(
                document,
                &mut main_table_builder,
                None,
            );
            let main_table = main_table_builder.into();
            tables.push(main_table);
        }

        for relation in document.relations.iter() {
            if relation.relation_type.is_owning() {
                let (working_relation, snapshot_relation) =
                    RelationTablesBuilder::new_pair(document, relation, documents);
                tables.push(working_relation);
                if document.has_draft_and_publish() {
                    tables.push(snapshot_relation);
                }
            }
        }

        Self { tables }
    }
}

struct MainTableBuilder {
    table_name: String,
    columns: Vec<Column>,
}

impl MainTableBuilder {
    fn new(document: &DocumentType) -> Self {
        let table_name = document.id.normalized();

        let mut columns = vec![
            Column::primary_key(DOCUMENT_ID_FIELD_NAME, ColumnType::Uuid, None),
            Column::new(
                STATUS_FIELD_NAME,
                ColumnType::Text,
                None,
                true,
                false,
                Some("'DRAFT'"),
            ),
            Column::new(
                VERSION_FIELD_NAME,
                ColumnType::Integer(IntegerSize::Int32),
                None,
                true,
                false,
                Some("1"),
            ),
        ];

        columns.extend(common_columns());

        Self {
            table_name,
            columns,
        }
    }

    fn push(&mut self, column: Column) {
        self.columns.push(column);
    }

    fn into(self) -> Table {
        let foreign_keys = vec![];
        let indexes = vec![];

        Table::new(self.table_name, self.columns, foreign_keys, indexes)
    }
}

struct SnapshotsTableBuilder {
    table_name: String,
    columns: Vec<Column>,
}

impl SnapshotsTableBuilder {
    fn new(document: &DocumentType) -> Self {
        let table_name = format!("{}_snapshots", document.id.normalized());
        let mut columns = vec![
            Column::primary_key(SNAPSHOT_ID_FIELD_NAME, ColumnType::Identity(IntegerSize::Int64), None),
            Column::new(
                DOCUMENT_ID_FIELD_NAME,
                ColumnType::Uuid,
                None,
                true,
                false,
                None,
            ),
        ];

        columns.extend(common_columns());

        Self {
            table_name,
            columns,
        }
    }

    fn push(&mut self, column: Column) {
        self.columns.push(column);
    }

    fn into(self) -> Table {
        let main_table_name = self.table_name.strip_suffix("_snapshots").unwrap();

        let foreign_keys = vec![ForeignKeyConstraint::new(
            &self.table_name as &str,
            DOCUMENT_ID_FIELD_NAME,
            main_table_name,
            DOCUMENT_ID_FIELD_NAME,
        )];

        let indexes = vec![Index::new(
            &self.table_name as &str,
            vec![DOCUMENT_ID_FIELD_NAME, REVISION_FIELD_NAME],
            true,
        )];

        Table::new(self.table_name, self.columns, foreign_keys, indexes)
    }
}

fn common_columns() -> Vec<Column> {
    vec![
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
            true,
            false,
            Some("now()"),
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
            REVISION_FIELD_NAME,
            ColumnType::Integer(IntegerSize::Int32),
            None,
            true,
            false,
            Some("0"),
        ),
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
    ]
}

struct RelationTablesBuilder;

impl RelationTablesBuilder {
    fn new_pair(
        document: &DocumentType,
        relation: &DocumentRelation,
        documents: &dyn DocumentTypesRegistry,
    ) -> (Table, Table) {
        let target_document = documents.get(&relation.target).unwrap();
        let target_table_name = target_document.id.normalized();
        let relation_table_name = format!(
            "{}_{}_relation",
            document.id.normalized(),
            relation.id.normalized()
        );
        let snapshot_relation_table_name = format!(
            "{}_{}_relation_snapshots",
            document.id.normalized(),
            relation.id.normalized()
        );

        // Working relation table
        let working_columns = vec![
            Column::primary_key(OWNING_DOCUMENT_ID_FIELD_NAME, ColumnType::Uuid, None),
            Column::primary_key(TARGET_DOCUMENT_ID_FIELD_NAME, ColumnType::Uuid, None),
        ];

        let working_foreign_keys = vec![
            ForeignKeyConstraint::new(
                &relation_table_name as &str,
                OWNING_DOCUMENT_ID_FIELD_NAME,
                &document.id.normalized(),
                DOCUMENT_ID_FIELD_NAME,
            ),
            ForeignKeyConstraint::new(
                &relation_table_name as &str,
                TARGET_DOCUMENT_ID_FIELD_NAME,
                &target_table_name,
                DOCUMENT_ID_FIELD_NAME,
            ),
        ];

        let working_indexes = vec![Index::new(
            &relation_table_name as &str,
            vec![TARGET_DOCUMENT_ID_FIELD_NAME],
            false,
        )];

        let working_table = Table::new(
            relation_table_name.clone(),
            working_columns,
            working_foreign_keys,
            working_indexes,
        );

        // Snapshot relation table
        let snapshot_columns = vec![
            Column::primary_key(
                SNAPSHOT_ID_FIELD_NAME,
                ColumnType::Integer(IntegerSize::Int64),
                None,
            ),
            Column::primary_key(TARGET_DOCUMENT_ID_FIELD_NAME, ColumnType::Uuid, None),
            Column::new(
                OWNING_DOCUMENT_ID_FIELD_NAME,
                ColumnType::Uuid,
                None,
                true,
                false,
                None,
            ),
        ];

        let snapshot_foreign_keys = vec![
            ForeignKeyConstraint::new(
                &snapshot_relation_table_name as &str,
                SNAPSHOT_ID_FIELD_NAME,
                &format!("{}_snapshots", document.id.normalized()),
                SNAPSHOT_ID_FIELD_NAME,
            ),
            ForeignKeyConstraint::new(
                &snapshot_relation_table_name as &str,
                TARGET_DOCUMENT_ID_FIELD_NAME,
                &target_table_name,
                DOCUMENT_ID_FIELD_NAME,
            ),
            ForeignKeyConstraint::new(
                &snapshot_relation_table_name as &str,
                OWNING_DOCUMENT_ID_FIELD_NAME,
                &document.id.normalized(),
                DOCUMENT_ID_FIELD_NAME,
            ),
        ];

        let snapshot_indexes = vec![
            Index::new(
                &snapshot_relation_table_name as &str,
                vec![TARGET_DOCUMENT_ID_FIELD_NAME],
                false,
            ),
            Index::new(
                &snapshot_relation_table_name as &str,
                vec![OWNING_DOCUMENT_ID_FIELD_NAME],
                false,
            ),
        ];

        let snapshot_table = Table::new(
            snapshot_relation_table_name,
            snapshot_columns,
            snapshot_foreign_keys,
            snapshot_indexes,
        );

        (working_table, snapshot_table)
    }
}

fn handle_document_fields(
    document: &DocumentType,
    main_table_builder: &mut MainTableBuilder,
    mut snapshots_table_builder: Option<&mut SnapshotsTableBuilder>,
) {
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

        main_table_builder.push(column.clone());
        if let Some(ref mut stb) = snapshots_table_builder {
            stb.push(column);
        }
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
