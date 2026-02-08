use docx_store::models::{DocBlock, Symbol};
use surrealdb::Connection;

use super::{ControlError, DocxControlPlane};

impl<C: Connection> DocxControlPlane<C> {
    pub async fn get_symbol(
        &self,
        project_id: &str,
        symbol_key: &str,
    ) -> Result<Option<Symbol>, ControlError> {
        Ok(self.store.get_symbol_by_project(project_id, symbol_key).await?)
    }

    pub async fn list_doc_blocks(
        &self,
        project_id: &str,
        symbol_key: &str,
        ingest_id: Option<&str>,
    ) -> Result<Vec<DocBlock>, ControlError> {
        Ok(self
            .store
            .list_doc_blocks(project_id, symbol_key, ingest_id)
            .await?)
    }

    pub async fn search_symbols(
        &self,
        project_id: &str,
        name: &str,
        limit: usize,
    ) -> Result<Vec<Symbol>, ControlError> {
        Ok(self
            .store
            .list_symbols_by_name(project_id, name, limit)
            .await?)
    }

    pub async fn search_doc_blocks(
        &self,
        project_id: &str,
        text: &str,
        limit: usize,
    ) -> Result<Vec<DocBlock>, ControlError> {
        Ok(self.store.search_doc_blocks(project_id, text, limit).await?)
    }

    pub async fn list_symbol_kinds(
        &self,
        project_id: &str,
    ) -> Result<Vec<String>, ControlError> {
        Ok(self.store.list_symbol_kinds(project_id).await?)
    }

    pub async fn list_members_by_scope(
        &self,
        project_id: &str,
        scope: &str,
        limit: usize,
    ) -> Result<Vec<Symbol>, ControlError> {
        Ok(self
            .store
            .list_members_by_scope(project_id, scope, limit)
            .await?)
    }
}
