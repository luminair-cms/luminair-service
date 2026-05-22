use std::collections::HashMap;

use luminair_common::{AttributeId, DocumentType};

use crate::application::DocumentsService;
use crate::domain::document::{
    DatabaseRowId, DocumentInstance, DocumentInstanceId,
    content::{ContentValue, DocumentContent},
};
use crate::domain::query::DocumentInstanceQuery;
use crate::domain::repository::{DocumentsRepository, RelationMap, RelationOps, RepositoryError};

#[derive(Clone)]
pub struct DocumentsServiceImpl<R>
where
    R: DocumentsRepository,
{
    repository: R,
}

impl<R: DocumentsRepository> DocumentsServiceImpl<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R: DocumentsRepository> DocumentsService for DocumentsServiceImpl<R> {
    async fn find(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        query: DocumentInstanceQuery,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let instances = self.repository.find(document_type, &query).await?;
        self.enrich(document_type, populate, instances).await
    }

    async fn find_by_id(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        query: DocumentInstanceQuery,
        id: DocumentInstanceId,
    ) -> Result<Option<DocumentInstance>, RepositoryError> {
        let opt = self
            .repository
            .find_by_id(document_type, id, &query)
            .await?;
        // Wrap in a Vec to reuse the batch enrichment helper, then unwrap.
        let instances = opt.into_iter().collect::<Vec<_>>();
        let enriched = self.enrich(document_type, populate, instances).await?;
        Ok(enriched.into_iter().next())
    }

    async fn create(
        &self,
        document_type: &DocumentType,
        fields: HashMap<AttributeId, ContentValue>,
    ) -> Result<DocumentInstanceId, RepositoryError> {
        let document_id = DocumentInstanceId::generate();
        let content = DocumentContent::new(fields);
        let instance = DocumentInstance::new(
            DatabaseRowId(0), // placeholder — the DB assigns the actual row key
            document_id,
            content,
            HashMap::new(),
        );
        self.repository.insert(document_type, &instance).await?;
        Ok(document_id)
    }

    async fn delete(
        &self,
        document_type: &DocumentType,
        id: DocumentInstanceId,
    ) -> Result<(), RepositoryError> {
        self.repository.delete(document_type, id).await
    }

    async fn modify_relations(
        &self,
        document_type: &DocumentType,
        document_id: DocumentInstanceId,
        ops: HashMap<AttributeId, RelationOps>,
    ) -> Result<(), RepositoryError> {
        // Validate all relation attributes before any DB operation.
        for attr_id in ops.keys() {
            let rel_meta = document_type.relations.get(attr_id).ok_or_else(|| {
                RepositoryError::ValidationFailed(format!("Relation not found: {}", attr_id))
            })?;
            if !rel_meta.relation_type.is_owning() {
                return Err(RepositoryError::ValidationFailed(format!(
                    "Relation '{}' is not an owning relation",
                    attr_id
                )));
            }
        }
        self.repository
            .apply_relation_ops(document_type, document_id, &ops)
            .await
    }
}

impl<R: DocumentsRepository> DocumentsServiceImpl<R> {
    /// Batch-load and attach relations to a set of document instances.
    ///
    /// If `populate` is `None` or the instance list is empty the documents are
    /// returned unchanged.
    async fn enrich(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        instances: Vec<DocumentInstance>,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let Some(fields) = populate else {
            return Ok(instances);
        };
        if instances.is_empty() || fields.is_empty() {
            return Ok(instances);
        }

        let row_ids: Vec<DatabaseRowId> = instances.iter().map(|d| d.id).collect();
        let relation_map: RelationMap = self
            .repository
            .fetch_relations(document_type, &row_ids, &fields)
            .await?;

        let enriched = instances
            .into_iter()
            .map(|instance| {
                let per_doc: HashMap<AttributeId, Vec<DocumentInstance>> = relation_map
                    .iter()
                    .map(|(attr_id, by_row)| {
                        let related = by_row.get(&instance.id).cloned().unwrap_or_default();
                        (attr_id.clone(), related)
                    })
                    .collect();
                instance.with_relations(per_doc)
            })
            .collect();

        Ok(enriched)
    }
}
