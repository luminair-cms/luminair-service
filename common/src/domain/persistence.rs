use crate::{AttributeId, DocumentType};

#[derive(Debug)]
pub struct QualifiedTable<'a> {
    pub name: TableNameProvider<'a>,
    pub alias: &'static str,
}

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
            Self::MainTable { document } => format!("{}", document.info.singular_name.normalized()),
            Self::RelationTable { document, relation } => format!(
                "{}_{}_relation",
                document.info.singular_name.normalized(),
                relation.normalized()
            ),
        }
    }
}

impl<'a> QualifiedTable<'a> {
    /// Get a qualified table name with alias
    pub fn qualified(&self) -> String {
        match self.name {
            TableNameProvider::MainTable { document } => format!(
                "\"{}\" AS \"{}\"",
                document.info.singular_name.normalized(),
                self.alias
            ),
            TableNameProvider::RelationTable { document, relation } => format!(
                "\"{}_{}_relation\" AS \"{}\"",
                document.info.singular_name.normalized(),
                relation.normalized(),
                self.alias
            ),
        }
    }
}

impl<'a> From<&'a DocumentType> for QualifiedTable<'a> {
    fn from(document: &'a DocumentType) -> Self {
        Self {
            name: TableNameProvider::MainTable { document },
            alias: "m",
        }
    }
}

impl<'a> From<(&'a DocumentType, &'a AttributeId)> for QualifiedTable<'a> {
    fn from(value: (&'a DocumentType, &'a AttributeId)) -> Self {
        let document = value.0;
        let relation = value.1;
        Self {
            name: TableNameProvider::RelationTable { document, relation },
            alias: "r",
        }
    }
}
