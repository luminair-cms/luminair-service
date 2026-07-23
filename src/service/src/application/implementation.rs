use crate::application::commands::{
    CreateDocumentCommand, CreateDocumentWithRelationsCommand, DeleteDocumentCommand,
    FindByIdCommand, FindDocumentsCommand, ModifyRelationsCommand, PublishDocumentCommand,
    RelationOperation, UpdateDocumentCommand, UpdateDocumentWithRelationsCommand,
};
use crate::application::error::ServiceError;
use crate::application::service::DocumentsService;
use crate::domain::document::content::DocumentContent;
use crate::domain::document::error::DocumentError;
use crate::domain::document::{
    DatabaseRowId, DocumentInstance, DocumentInstanceId, lifecycle::PublicationState,
};
use crate::domain::query::{DocumentInstanceQuery, DocumentStatus};
use crate::domain::repository::{DocumentsRepository, RelationMap, RelationOps, RepositoryError};
use chrono::Utc;
use luminair_common::{AttributeId, DocumentType};
use std::collections::HashMap;

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

    /// Batch-load and attach relations to a set of document instances.
    ///
    /// If `populate` is `None` or the instance list is empty the documents are
    /// returned unchanged.
    async fn enrich(
        &self,
        document_type: &DocumentType,
        populate: Option<Vec<AttributeId>>,
        populate_filters: Option<HashMap<AttributeId, crate::domain::query::FilterExpression>>,
        status: DocumentStatus,
        instances: Vec<DocumentInstance>,
    ) -> Result<Vec<DocumentInstance>, RepositoryError> {
        let Some(fields) = populate else {
            return Ok(instances);
        };
        if instances.is_empty() || fields.is_empty() {
            return Ok(instances);
        }

        let ids: Vec<DocumentInstanceId> = instances.iter().map(|d| d.document_id).collect();
        let empty_filters = HashMap::new();
        let filters = populate_filters.as_ref().unwrap_or(&empty_filters);

        let relation_map: RelationMap = self
            .repository
            .fetch_relations(document_type, &fields, filters, status, &ids)
            .await?;

        let enriched = instances
            .into_iter()
            .map(|instance| {
                let per_doc: HashMap<AttributeId, Vec<DocumentInstance>> = relation_map
                    .iter()
                    .map(|(attr_id, by_row)| {
                        let related = by_row
                            .get(&instance.document_id)
                            .cloned()
                            .unwrap_or_default();
                        (attr_id.clone(), related)
                    })
                    .collect();
                instance.with_relations(per_doc)
            })
            .collect();

        Ok(enriched)
    }
}

impl<R: DocumentsRepository> DocumentsService for DocumentsServiceImpl<R> {
    async fn find(
        &self,
        cmd: FindDocumentsCommand,
    ) -> Result<(Vec<DocumentInstance>, u64), ServiceError> {
        let (instances, count) = tokio::try_join!(
            self.repository.find(cmd.document_type, &cmd.query),
            self.repository.count(cmd.document_type, &cmd.query),
        )?;
        let enriched = self
            .enrich(
                cmd.document_type,
                cmd.populate,
                cmd.populate_filters,
                cmd.query.status,
                instances,
            )
            .await?;
        Ok((enriched, count))
    }

    async fn find_by_id(
        &self,
        cmd: FindByIdCommand,
    ) -> Result<Option<DocumentInstance>, ServiceError> {
        let opt = self
            .repository
            .find_by_id(cmd.document_type, cmd.document_instance_id, &cmd.query)
            .await?;
        let instance = match opt {
            Some(inst) => inst,
            None => return Ok(None),
        };
        let enriched = self
            .enrich(
                cmd.document_type,
                cmd.populate,
                cmd.populate_filters,
                cmd.query.status,
                vec![instance],
            )
            .await?;

        Ok(enriched.into_iter().next())
    }

    async fn create(&self, cmd: CreateDocumentCommand) -> Result<DocumentInstanceId, ServiceError> {
        // ContentValue::from_json catches explicit-null on required fields at parse time, 
        // but cannot see fields omitted from the payload altogether — closing that gap is the service's job.
        for field in &cmd.document_type.fields {
            if field.required && !cmd.fields.contains_key(&field.id) {
                return Err(ServiceError::Validation(
                    DocumentError::MissingRequiredField(field.id.to_string()),
                ));
            }
        }

        let document_id = DocumentInstanceId::generate();
        let content = DocumentContent::new(cmd.fields);
        let instance = DocumentInstance::new(
            DatabaseRowId(0), // placeholder — the DB assigns the actual row key
            document_id,
            content,
            HashMap::new(),
        );
        self.repository.insert(cmd.document_type, &instance).await?;
        Ok(document_id)
    }

    async fn create_with_relations(
        &self,
        cmd: CreateDocumentWithRelationsCommand,
    ) -> Result<DocumentInstanceId, ServiceError> {
        let create_cmd = CreateDocumentCommand {
            document_type: cmd.document_type,
            fields: cmd.fields,
            user_id: cmd.user_id.clone(),
        };
        let created_id = self.create(create_cmd).await?;

        if !cmd.relation_operations.is_empty() {
            let modify_cmd = ModifyRelationsCommand {
                document_type: cmd.document_type,
                document_id: created_id,
                operations: cmd.relation_operations,
            };
            self.modify_relations(modify_cmd).await?;
        }

        Ok(created_id)
    }

    async fn update(&self, cmd: UpdateDocumentCommand) -> Result<(), ServiceError> {
        // Updates are applied to the draft row — the published row is immutable
        // until the next `publish()` call propagates the draft forward.
        let query = DocumentInstanceQuery::new().with_status(DocumentStatus::Draft);
        let mut instance = self
            .repository
            .find_by_id(cmd.document_type, cmd.document_id, &query)
            .await?
            .ok_or(ServiceError::DocumentNotFound)?;

        instance.content.fields.extend(cmd.fields);
        instance.audit.version += 1;
        instance.audit.updated_at = Utc::now();
        instance.audit.updated_by = cmd.user_id;

        // Transition publication state to Draft (MODIFIED editorial status) if it's currently Published
        if let PublicationState::Published { revision, .. } = &instance.content.publication_state {
            instance.content.publication_state = PublicationState::Draft {
                revision: *revision,
            };
        }

        self.repository.update(cmd.document_type, &instance).await?;
        Ok(())
    }

    async fn update_with_relations(
        &self,
        cmd: UpdateDocumentWithRelationsCommand,
    ) -> Result<(), ServiceError> {
        if !cmd.fields.is_empty() {
            let update_cmd = UpdateDocumentCommand {
                document_type: cmd.document_type,
                document_id: cmd.document_id,
                fields: cmd.fields,
                user_id: cmd.user_id.clone(),
            };
            self.update(update_cmd).await?;
        }

        if !cmd.relation_operations.is_empty() {
            let modify_cmd = ModifyRelationsCommand {
                document_type: cmd.document_type,
                document_id: cmd.document_id,
                operations: cmd.relation_operations,
            };
            self.modify_relations(modify_cmd).await?;
        }

        Ok(())
    }

    async fn delete(&self, cmd: DeleteDocumentCommand) -> Result<(), ServiceError> {
        self.repository
            .delete(cmd.document_type, cmd.document_instance_id)
            .await
            .map_err(ServiceError::from)
    }

    async fn publish(&self, cmd: PublishDocumentCommand) -> Result<(), ServiceError> {
        // Publish always operates on the draft row — the state machine lives in
        // `DocumentInstance::publish`, the repository only persists the result.
        // TODO: if the document is already published, this will return an AlreadyPublished error.
        let query = DocumentInstanceQuery::new().with_status(DocumentStatus::Draft);
        let mut instance = self
            .repository
            .find_by_id(cmd.document_type, cmd.document_id, &query)
            .await?
            .ok_or(ServiceError::DocumentNotFound)?;

        instance.publish(cmd.user_id.clone())?;
        instance.audit.updated_at = Utc::now();
        instance.audit.updated_by = cmd.user_id;

        self.repository.update(cmd.document_type, &instance).await?;
        Ok(())
    }

    async fn modify_relations(&self, cmd: ModifyRelationsCommand) -> Result<(), ServiceError> {
        // Validate every targeted attribute is an owning relation, then convert
        // the command-layer `RelationOperation` enum into the repository's
        // `RelationOps` struct in a single pass — all validation happens before
        // any DB call so a bad payload never causes a partial write.
        let mut ops: HashMap<AttributeId, RelationOps> =
            HashMap::with_capacity(cmd.operations.len());
        for (attr_id, operation) in cmd.operations {
            let rel_meta = cmd
                .document_type
                .relations
                .get(&attr_id)
                .ok_or_else(|| ServiceError::RelationNotFound(attr_id.to_string()))?;
            if !rel_meta.relation_type.is_owning() {
                return Err(ServiceError::NotOwningRelation(attr_id.to_string()));
            }
            let rel_ops = match operation {
                RelationOperation::ConnectDisconnect {
                    connect,
                    disconnect,
                } => RelationOps {
                    connect,
                    disconnect,
                },
                // Full-replacement semantics land in Phase 5 (queries/relations.rs):
                // the diff against the existing set needs DB access to compute.
                RelationOperation::Set(_) => {
                    return Err(ServiceError::Internal(anyhow::anyhow!(
                        "`set` relation operation is not yet supported"
                    )));
                }
            };
            ops.insert(attr_id, rel_ops);
        }
        self.repository
            .apply_relation_ops(cmd.document_type, cmd.document_id, &ops)
            .await
            .map_err(ServiceError::from)?;

        // Fetch draft/working copy of the document
        let query = DocumentInstanceQuery::new().with_status(DocumentStatus::Draft);
        let mut instance = self
            .repository
            .find_by_id(cmd.document_type, cmd.document_id, &query)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::DocumentNotFound)?;

        // Bump the version and transition status (e.g. from PUBLISHED to MODIFIED)
        instance.audit.version += 1;
        instance.audit.updated_at = Utc::now();

        // Transition publication state to Draft (MODIFIED editorial status) if it's currently Published
        if let PublicationState::Published { revision, .. } = &instance.content.publication_state {
            instance.content.publication_state = PublicationState::Draft {
                revision: *revision,
            };
        }

        self.repository
            .update(cmd.document_type, &instance)
            .await
            .map_err(ServiceError::from)?;

        Ok(())
    }
}
