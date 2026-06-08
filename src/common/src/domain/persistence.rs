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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DocumentTypeId, AttributeId};
    use crate::entities::{DocumentKind, DocumentTitle, DocumentTypeInfo};

    fn make_doc(id: &str) -> DocumentType {
        DocumentType {
            id: DocumentTypeId::try_new(id).unwrap(),
            kind: DocumentKind::Collection,
            info: DocumentTypeInfo {
                title: DocumentTitle::try_new("T").unwrap(),
                singular_name: DocumentTypeId::try_new(id).unwrap(),
                plural_name: DocumentTypeId::try_new(format!("{}s", id).as_str()).unwrap(),
                description: None,
            },
            options: None,
            fields: Default::default(),
            relations: Default::default(),
        }
    }

    #[test]
    fn table_name_and_qualified() {
        let doc = make_doc("product");
        let provider = TableNameProvider::from(&doc);
        assert_eq!(provider.table_name(), "product");
        assert_eq!(provider.alias(), "m");
        assert_eq!(provider.qualified(), "product AS \"m\"");

        let attr = AttributeId::try_new("owner").unwrap();
        let rel = TableNameProvider::from((&doc, &attr));
        assert_eq!(rel.table_name(), "product_owner_relation");
        assert_eq!(rel.alias(), "r");
        assert_eq!(rel.qualified(), "product_owner_relation AS \"r\"");
    }
}
