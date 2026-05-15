use crate::domain::application::DocumentsService;
use crate::domain::document::content::{ContentValue, DocumentContent};
use crate::domain::document::{DatabaseRowId, DocumentInstance, DocumentInstanceId};
use crate::domain::repository::query::DocumentInstanceQuery;
use crate::domain::repository::{DocumentsRepository, RepositoryError};
use luminair_common::{AttributeId, DocumentType};
use std::collections::HashMap;

#[derive(Clone)]
pub struct DocumentsServiceImpl<R>
where
    R: DocumentsRepository
{
    repository: R,
}

impl<R> DocumentsServiceImpl<R>
where
    R: DocumentsRepository
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R> DocumentsService for DocumentsServiceImpl<R>
where
    R: DocumentsRepository
{
    async fn find(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        query: DocumentInstanceQuery,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let mut document_instances = self.repository.find(document_type, query).await?;

        let result = if !document_instances.is_empty()
            && let Some(populate_fields) = populate
        {
            // Collect all instance IDs for batch fetching
            let ids: Vec<DatabaseRowId> = document_instances
                .iter()
                .map(|doc| DatabaseRowId::try_from(doc.id).unwrap())
                .collect();

            // Fetch all relations for this batch of documents
            let all_relations = self
                .repository
                .fetch_relations_for_many(document_type, &ids, &populate_fields)
                .await?;

            // Apply relations to each document instance
            for document in &mut document_instances {
                let id = DatabaseRowId::from(document.id);
                let doc_relations: HashMap<AttributeId, Vec<DocumentInstance>> = all_relations
                    .iter()
                    .filter_map(|(attr_id, related_docs_by_id)| {
                        let related_responses: Vec<DocumentInstance> = related_docs_by_id
                            .get(&id)
                            .map(|instances| instances.iter().cloned().map(Into::into).collect())
                            .unwrap_or_default();
                        Some((attr_id.clone(), related_responses))
                    })
                    .collect();
                // TODO: improve performance by avoiding cloning
                *document = document.clone().with_relations(doc_relations);
            }
            document_instances
        } else {
            document_instances
        };

        Ok(result)
    }

    async fn find_by_id(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        query: DocumentInstanceQuery,
        id: DocumentInstanceId,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let mut document_instances = self
            .repository
            .find_by_id(document_type, query, id)
            .await?;

        if !document_instances.is_empty() {
            if let Some(populate_fields) = populate {
                let ids: Vec<DatabaseRowId> = document_instances
                    .iter()
                    .map(|doc| doc.id)
                    .collect();

                let all_relations = self
                    .repository
                    .fetch_relations_for_many(document_type, &ids, &populate_fields)
                    .await?;

                for document in &mut document_instances {
                    let id = document.id;
                    let doc_relations: HashMap<AttributeId, Vec<DocumentInstance>> = all_relations
                        .iter()
                        .filter_map(|(attr_id, related_docs_by_id)| {
                            let related_responses: Vec<DocumentInstance> = related_docs_by_id
                                .get(&id)
                                .map(|instances| instances.iter().cloned().map(Into::into).collect())
                                .unwrap_or_default();
                            Some((attr_id.clone(), related_responses))
                        })
                        .collect();

                    *document = document.clone().with_relations(doc_relations);
                }
            }
        }

        Ok(document_instances)
    }

    async fn create(
        &self,
        document_type: &DocumentType,
        fields: HashMap<AttributeId, ContentValue>,
    ) -> Result<DocumentInstanceId, RepositoryError> {
        let content = DocumentContent::new(fields);

        self.repository.create(document_type, content, None).await
    }

    async fn delete(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> Result<(), RepositoryError> {
        self.repository.delete(document_type, id).await?;
        Ok(())
    }

    async fn connect(
        &self,
        document_type: &DocumentType,
        relation_attr: &AttributeId,
        owning_id: DatabaseRowId,
        inverse_id: DatabaseRowId,
    ) -> Result<(), RepositoryError> {
        self.repository
            .connect(document_type, relation_attr, owning_id, inverse_id)
            .await
    }

    async fn disconnect(
        &self,
        document_type: &DocumentType,
        relation_attr: &AttributeId,
        owning_id: DatabaseRowId,
        inverse_id: DatabaseRowId,
    ) -> Result<(), RepositoryError> {
        self.repository
            .disconnect(document_type, relation_attr, owning_id, inverse_id)
            .await
    }
}
