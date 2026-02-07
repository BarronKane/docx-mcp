use std::{collections::HashMap, error::Error, fmt};

use docx_store::models::{DocBlock, DocSource, RelationRecord, Symbol};
use docx_store::schema::{REL_DOCUMENTS, SOURCE_KIND_CSHARP_XML};
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};

use crate::parsers::{CsharpParseError, CsharpParseOptions, CsharpXmlParser};
use crate::store::{StoreError, SurrealDocStore};

#[derive(Debug)]
pub enum ControlError {
    Parse(CsharpParseError),
    Store(StoreError),
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "{err}"),
            Self::Store(err) => write!(f, "{err}"),
        }
    }
}

impl Error for ControlError {}

impl From<CsharpParseError> for ControlError {
    fn from(err: CsharpParseError) -> Self {
        Self::Parse(err)
    }
}

impl From<StoreError> for ControlError {
    fn from(err: StoreError) -> Self {
        Self::Store(err)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsharpIngestRequest {
    pub project_id: String,
    pub xml: String,
    pub ingest_id: Option<String>,
    pub source_path: Option<String>,
    pub source_modified_at: Option<String>,
    pub tool_version: Option<String>,
    pub source_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsharpIngestReport {
    pub assembly_name: Option<String>,
    pub symbol_count: usize,
    pub doc_block_count: usize,
    pub documents_edge_count: usize,
    pub doc_source_id: Option<String>,
}

#[derive(Clone)]
pub struct DocxControlPlane<C: Connection> {
    store: SurrealDocStore<C>,
}

impl<C: Connection> DocxControlPlane<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            store: SurrealDocStore::new(db),
        }
    }

    pub fn with_store(store: SurrealDocStore<C>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &SurrealDocStore<C> {
        &self.store
    }

    pub async fn ingest_csharp_xml(
        &self,
        request: CsharpIngestRequest,
    ) -> Result<CsharpIngestReport, ControlError> {
        let CsharpIngestRequest {
            project_id,
            xml,
            ingest_id,
            source_path,
            source_modified_at,
            tool_version,
            source_hash,
        } = request;

        if project_id.trim().is_empty() {
            return Err(ControlError::Store(StoreError::InvalidInput(
                "project_id is required".to_string(),
            )));
        }

        let mut options = CsharpParseOptions::new(project_id.clone());
        if let Some(ref ingest_id) = ingest_id {
            options = options.with_ingest_id(ingest_id.clone());
        }

        let parsed = CsharpXmlParser::parse_async(xml, options).await?;

        let mut stored_symbols = Vec::with_capacity(parsed.symbols.len());
        for symbol in parsed.symbols {
            stored_symbols.push(self.store.upsert_symbol(symbol).await?);
        }

        let stored_blocks = self.store.create_doc_blocks(parsed.doc_blocks).await?;

        let doc_source_id = if source_path.is_some()
            || tool_version.is_some()
            || source_hash.is_some()
            || source_modified_at.is_some()
        {
            let source = DocSource {
                id: None,
                project_id: project_id.clone(),
                ingest_id: ingest_id.clone(),
                language: Some("csharp".to_string()),
                source_kind: Some(SOURCE_KIND_CSHARP_XML.to_string()),
                path: source_path,
                tool_version,
                hash: source_hash,
                source_modified_at,
                extra: None,
            };
            let created = self.store.create_doc_source(source).await?;
            created.id
        } else {
            None
        };

        let documents = build_documents_edges(
            &stored_symbols,
            &stored_blocks,
            &project_id,
            ingest_id.as_deref(),
        );
        let documents_edge_count = documents.len();
        if !documents.is_empty() {
            let _ = self.store.create_relations(REL_DOCUMENTS, documents).await?;
        }

        Ok(CsharpIngestReport {
            assembly_name: parsed.assembly_name,
            symbol_count: stored_symbols.len(),
            doc_block_count: stored_blocks.len(),
            documents_edge_count,
            doc_source_id,
        })
    }

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
}

fn build_documents_edges(
    symbols: &[Symbol],
    blocks: &[DocBlock],
    project_id: &str,
    ingest_id: Option<&str>,
) -> Vec<RelationRecord> {
    let mut symbol_map = HashMap::new();
    for symbol in symbols {
        if let Some(id) = symbol.id.as_ref() {
            symbol_map.insert(symbol.symbol_key.as_str(), id.clone());
        }
    }

    let mut relations = Vec::new();
    for block in blocks {
        let Some(block_id) = block.id.as_ref() else {
            continue;
        };
        let Some(symbol_key) = block.symbol_key.as_ref() else {
            continue;
        };
        let Some(symbol_id) = symbol_map.get(symbol_key.as_str()) else {
            continue;
        };
        relations.push(RelationRecord {
            id: None,
            in_id: block_id.clone(),
            out_id: symbol_id.clone(),
            project_id: project_id.to_string(),
            ingest_id: ingest_id.map(str::to_string),
            kind: None,
            extra: None,
        });
    }
    relations
}
