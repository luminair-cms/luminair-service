use std::borrow::Cow;
use std::collections::HashMap;

use crate::domain::DocumentRowId;
use crate::persistence::database::{Condition, DatabaseQuery, Select, Table, DOCUMENT_ID_COLUMN};
use anyhow::Error;
use luminair_common::database::Database;
use luminair_common::documents::AttributeId;
use luminair_common::documents::attributes::DocumentRelation;
use luminair_common::documents::documents::Document;
use serde::Serialize;

pub fn find_by_id(
    document: &'static Document,
    populate: HashMap<AttributeId, (&'static DocumentRelation, &'static Document)>,
    document_id: DocumentRowId,
) -> DocumentQuery {
    DocumentQuery {
        document,
        populate,
        filter: Some(QueryFilter::ById(document_id)),
        pagination: None,
    }
}

pub fn find_all(
    document: &'static Document,
    populate: HashMap<AttributeId, (&'static DocumentRelation, &'static Document)>,
    pagination: Option<Pagination>,
) -> DocumentQuery {
    DocumentQuery {
        document,
        populate,
        filter: None,
        pagination: Some(pagination.unwrap_or_default()),
    }
}

#[derive(Clone, Debug)]
pub struct DocumentQuery {
    document: &'static Document,
    populate: HashMap<AttributeId, (&'static DocumentRelation, &'static Document)>,
    filter: Option<QueryFilter>,
    pagination: Option<Pagination>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRow {
    pub document_id: DocumentRowId,
    #[serde(flatten)]
    pub fields: HashMap<String,AttributeData>
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AttributeData {
    Field(String),
    LocalizedField(HashMap<String,String>),
    Relation(Vec<DocumentRow>)
}

#[derive(Clone, Debug)]
struct Pagination {
    page: u16,
    page_size: u16,
}

#[derive(Clone, Debug)]
struct Populate {
    relation: &'static DocumentRelation,
    target: &'static Document,
}

#[derive(Clone, Debug)]
enum QueryFilter {
    ById(DocumentRowId),
    
}

impl DocumentQuery {
    pub async fn execute(
        self,
        database: &'static Database,
    ) -> Result<Vec<DocumentRow>, Error> {
        let query = self.main_query();
        
        let result_set = query.execute(database).await?;
        
        // TODO: convert result_set into Documents
        // TODO: populate
        
        todo!()
    }
    
    fn main_query(&self) -> DatabaseQuery<'_> {
        let from = Table {
            name: &self.document.persistence.main_table_name,
            alias: "m",
        };
        let mut select = Select::new(self.document.has_draft_and_publish());
        for field in self.document.fields.values() {
            select.push(&field.table_column_name, "m");
        }
        
        let mut query = DatabaseQuery::new(from, select);
        
        if let Some(ref filter) = self.filter {
            match filter {
                QueryFilter::ById(document_row_id) => {
                    let id: i32 = document_row_id.into();
                    let condition = Condition::Equals(id);
                    query.condition(Cow::Borrowed(&DOCUMENT_ID_COLUMN), condition);
                }
            }
        }
        
        if let Some(ref pagination) = self.pagination {
            let offset = pagination.page * pagination.page_size;
            let next = pagination.page_size;
            query.pagination(offset, next);
        }
        
        query
    }
    
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 25,
        }
    }
}
