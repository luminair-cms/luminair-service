use crate::application::commands::{
    CreateDocumentCommand, CreateDocumentWithRelationsCommand, DeleteDocumentCommand,
    FindByIdCommand, FindDocumentsCommand, ModifyRelationsCommand, PublishDocumentCommand,
    UpdateDocumentCommand, UpdateDocumentWithRelationsCommand,
};
use crate::application::error::ServiceError;
use crate::domain::document::{DocumentInstance, DocumentInstanceId};

pub trait DocumentsService: Send + Sync + 'static {
    /// Returns (documents, total_count). total_count is used for pagination metadata.
    fn find(
        &self,
        cmd: FindDocumentsCommand,
    ) -> impl Future<Output = Result<(Vec<DocumentInstance>, u64), ServiceError>> + Send;

    fn find_by_id(
        &self,
        cmd: FindByIdCommand,
    ) -> impl Future<Output = Result<Option<DocumentInstance>, ServiceError>> + Send;

    fn create(
        &self,
        cmd: CreateDocumentCommand,
    ) -> impl Future<Output = Result<DocumentInstanceId, ServiceError>> + Send;

    fn create_with_relations(
        &self,
        cmd: CreateDocumentWithRelationsCommand,
    ) -> impl Future<Output = Result<DocumentInstanceId, ServiceError>> + Send;

    fn update(
        &self,
        cmd: UpdateDocumentCommand,
    ) -> impl Future<Output = Result<DocumentInstance, ServiceError>> + Send;

    fn update_with_relations(
        &self,
        cmd: UpdateDocumentWithRelationsCommand,
    ) -> impl Future<Output = Result<DocumentInstance, ServiceError>> + Send;

    fn delete(
        &self,
        cmd: DeleteDocumentCommand,
    ) -> impl Future<Output = Result<(), ServiceError>> + Send;

    fn publish(
        &self,
        cmd: PublishDocumentCommand,
    ) -> impl Future<Output = Result<DocumentInstance, ServiceError>> + Send;

    fn modify_relations(
        &self,
        cmd: ModifyRelationsCommand,
    ) -> impl Future<Output = Result<(), ServiceError>> + Send;
}
