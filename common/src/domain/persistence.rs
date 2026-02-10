use crate::{AttributeId, DocumentType};

#[derive(Debug)]
pub struct QualifiedTable {
    pub name: TableNameProvider,
    pub alias: &'static str,
}

#[derive(Debug)]
pub enum TableNameProvider {
    MainTable {
        document: &'static DocumentType,
    },
    RelationTable {
        document: &'static DocumentType,
        relation: &'static AttributeId,
    },
}

impl TableNameProvider {
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

impl QualifiedTable {
    /// Get qualified table name with alias
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

impl From<&'static DocumentType> for QualifiedTable {
    fn from(document: &'static DocumentType) -> Self {
        Self {
            name: TableNameProvider::MainTable { document },
            alias: "m",
        }
    }
}

impl From<(&'static DocumentType, &'static AttributeId)> for QualifiedTable {
    fn from(value: (&'static DocumentType, &'static AttributeId)) -> Self {
        let document = value.0;
        let relation = value.1;
        Self {
            name: TableNameProvider::RelationTable { document, relation },
            alias: "m",
        }
    }
}
