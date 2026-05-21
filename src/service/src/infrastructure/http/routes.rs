use crate::domain::application::AppState;
use crate::infrastructure::http::handlers::data::{
    create_new_document, delete_existing_document, find_all_documents, find_document_by_id,
    modify_relations,
};
use crate::infrastructure::http::handlers::documents::{documents_metadata, one_document_metadata};
use axum::Router;
use axum::routing::{delete, get, post, put};

pub fn api_routes<S: AppState>() -> Router<S> {
    Router::new()
        .route("/meta/documents", get(documents_metadata::<S>))
        .route("/meta/documents/{id}", get(one_document_metadata::<S>))
        .route("/documents/{api_type}", get(find_all_documents::<S>))
        .route("/documents/{api_type}/{id}", get(find_document_by_id::<S>))
        .route("/documents/{api_type}", post(create_new_document::<S>))
        .route(
            "/documents/{api_type}/{id}",
            delete(delete_existing_document::<S>),
        )
        .route("/documents/{api_type}/{id}", put(modify_relations::<S>))
}
