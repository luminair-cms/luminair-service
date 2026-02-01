use crate::domain::DocumentRowId;
use anyhow::Error;
use luminair_common::database::Database;
use luminair_common::documents::attributes::DocumentRelation;
use luminair_common::documents::documents::Document;

pub trait DocumentQuery: Clone + Send + Sync + 'static {
    fn select(self, database: &'static Database) -> impl Future<Output = Result<DocumentQueryResponse, Error>> + Send;

   fn find_by_id(document: &'static Document, document_id: DocumentRowId) -> impl DocumentQuery {
       FindById { document, document_id }
   }

    fn find_all(document: &'static Document, pagination: Option<Pagination>) -> impl DocumentQuery {
        FindAll { document, pagination: pagination.unwrap_or_default() }
    }

    fn find_related(main_document: &'static Document,
                    related_document: &'static Document,
                    relation: &'static DocumentRelation,
                    document_ids_list: Vec<DocumentRowId>) -> impl DocumentQuery {
        FindRelated { main_document, related_document, relation, document_ids_list }
    }
}

pub struct DocumentQueryResponse {

}

#[derive(Clone)]
pub struct Pagination {
    pub page: u16,
    pub page_ize: u16
}

impl Default for Pagination {
    fn default() -> Self {
        Self { page: 1, page_ize: 25 }
    }
}

#[derive(Clone)]
struct FindById {
    pub document: &'static Document,
    pub document_id: DocumentRowId
}

#[derive(Clone)]
struct FindAll {
    pub document: &'static Document,
    pub pagination: Pagination
}

#[derive(Clone)]
struct FindRelated {
    pub main_document: &'static Document,
    pub related_document: &'static Document,
    pub relation: &'static DocumentRelation,
    pub document_ids_list: Vec<DocumentRowId>
}

impl DocumentQuery for FindById {
    async fn select(self, database: &'static Database) -> Result<DocumentQueryResponse, Error> {
        todo!()
    }
}

impl DocumentQuery for FindAll {
    async fn select(self, database: &'static Database) -> Result<DocumentQueryResponse, Error> {
        todo!()
    }
}

impl DocumentQuery for FindRelated {
    async fn select(self, database: &'static Database) -> Result<DocumentQueryResponse, Error> {
        todo!()
    }
}