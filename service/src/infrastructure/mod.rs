use crate::{
    domain::{AppState, HelloService},
    infrastructure::persistence::PersistenceAdapter,
};
use anyhow::anyhow;
use luminair_common::domain::Documents;
use luminair_common::infrastructure::database::Database;

pub mod http;
pub mod persistence;
pub mod settings;

#[derive(Clone, Debug)]
pub struct HelloServiceAdapter {
    database: &'static Database,
}

impl HelloServiceAdapter {
    pub fn new(database: &'static Database) -> Self {
        Self { database }
    }
}

impl HelloService for HelloServiceAdapter {
    async fn hello(&self) -> Result<String, anyhow::Error> {
        sqlx::query_scalar("select 'hello world from pg'")
            .fetch_one(self.database.database_pool())
            .await
            .map_err(|e| anyhow!("failed to execute query: {}", e))
    }
}

#[derive(Clone)]
pub struct AppStateImpl {
    hello: HelloServiceAdapter,
    documents: &'static dyn Documents,
    persistence: PersistenceAdapter,
}

impl AppStateImpl {
    pub fn new(
        hello: HelloServiceAdapter,
        documents: &'static dyn Documents,
        persistence: PersistenceAdapter,
    ) -> Self {
        Self {
            hello,
            documents,
            persistence,
        }
    }
}

impl AppState for AppStateImpl {
    type H = HelloServiceAdapter;
    type P = PersistenceAdapter;

    fn hello_service(&self) -> &Self::H {
        &self.hello
    }

    fn documents(&self) -> &'static dyn Documents {
        self.documents
    }

    fn persistence(&self) -> &Self::P {
        &self.persistence
    }
}
