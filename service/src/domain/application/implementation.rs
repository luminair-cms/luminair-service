use crate::domain::application::DocumentServices;
use crate::domain::document::content::{ContentValue, DocumentContent};
use crate::domain::document::{DatabaseRowId, DocumentInstance, DocumentInstanceId};
use crate::domain::repository::query::DocumentInstanceQuery;
use crate::domain::repository::{DocumentsRepository, RepositoryError};
use luminair_common::{AttributeId, DocumentType, DocumentTypesRegistry};
use std::collections::HashMap;

#[derive(Clone)]
pub struct DocumentServicesImpl<R>
where
    R: DocumentsRepository
{
    repository: R,
    types: &'static dyn DocumentTypesRegistry,
}

impl<R> DocumentServicesImpl<R>
where
    R: DocumentsRepository
{
    pub fn new(repository: R, types: &'static dyn DocumentTypesRegistry) -> Self {
        Self {
            repository,
            types,
        }
    }
}

impl<R> DocumentServices for DocumentServicesImpl<R>
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
        id: DocumentInstanceId,
    ) -> Result<Option<DocumentInstance>, RepositoryError> {
        let document_instance_result = self.repository.find_by_id(document_type, id).await?;

        let result = if let Some(populate_fields) = populate
            && let Some(mut document_instance) = document_instance_result
        {
            let main_table_id = DatabaseRowId::from(document_instance.id);
            let relations = self
                .repository
                .fetch_relations_for_one(&document_type, main_table_id, &populate_fields)
                .await?;
            document_instance = document_instance.with_relations(relations);
            Some(document_instance)
        } else {
            document_instance_result
        };

        Ok(result)
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
}
