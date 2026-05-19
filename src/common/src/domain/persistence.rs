use sea_query::{IntoIden, TableName, TableRef};
use crate::{AttributeId, DocumentType};

#[derive(Debug)]
pub enum TableNameProvider<'a> {
    MainTable {
        document: &'a DocumentType,
    },
    RelationTable {
        document: &'a DocumentType,
        relation: &'a AttributeId,
    },
}

impl<'a> TableNameProvider<'a> {
    pub fn table_name(&self) -> String {
        match self {
            Self::MainTable { document } => format!("{}", document.id.normalized()),
            Self::RelationTable { document, relation } => format!(
                "{}_{}_relation",
                document.id.normalized(),
                relation.normalized()
            ),
        }
    }
    
    const MAIN_TABLE_ALIAS: &'static str = "m";
    const RELATION_TABLE_ALIAS: &'static str = "r";
    
    pub fn alias(&self) -> &'static str {
        match self {
            Self::MainTable { .. } => Self::MAIN_TABLE_ALIAS,
            Self::RelationTable { .. } => Self::RELATION_TABLE_ALIAS,
        }
    }

    pub fn qualified(&self) -> String {
        format!("{} AS \"{}\"", self.table_name(), self.alias())
    }
}

impl<'a> From<&'a DocumentType> for TableNameProvider<'a> {
    fn from(value: &'a DocumentType) -> Self {
        Self::MainTable { document: value }
    }
}

impl<'a> From<(&'a DocumentType, &'a AttributeId)> for TableNameProvider<'a> {
    fn from(value: (&'a DocumentType, &'a AttributeId)) -> Self {
        let document = value.0;
        let relation = value.1;
        TableNameProvider::RelationTable { document, relation }
    }
}

impl <'a> From <TableNameProvider<'a>> for TableRef {
    fn from(value: TableNameProvider<'a>) -> Self {
        TableRef::Table(
            TableName::from(value.table_name()), 
            Some(value.alias().into_iden())
        )
    }
}
