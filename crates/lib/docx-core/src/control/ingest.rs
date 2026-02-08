use std::collections::HashMap;

use docx_store::models::{DocBlock, DocSource, RelationRecord, Symbol};
use docx_store::schema::{REL_DOCUMENTS, SOURCE_KIND_CSHARP_XML, SOURCE_KIND_RUSTDOC_JSON};
use serde::{Deserialize, Serialize};
use surrealdb::Connection;

use crate::parsers::{CsharpParseOptions, CsharpXmlParser, RustdocJsonParser, RustdocParseOptions};
use crate::store::StoreError;

use super::{ControlError, DocxControlPlane};
use super::metadata::ProjectUpsertRequest;

/// Input payload for ingesting C# XML documentation.
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

/// Summary of a C# XML ingest operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsharpIngestReport {
    pub assembly_name: Option<String>,
    pub symbol_count: usize,
    pub doc_block_count: usize,
    pub documents_edge_count: usize,
    pub doc_source_id: Option<String>,
}

/// Input payload for ingesting rustdoc JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustdocIngestRequest {
    pub project_id: String,
    pub json: String,
    pub ingest_id: Option<String>,
    pub source_path: Option<String>,
    pub source_modified_at: Option<String>,
    pub tool_version: Option<String>,
    pub source_hash: Option<String>,
}

/// Summary of a rustdoc JSON ingest operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustdocIngestReport {
    pub crate_name: Option<String>,
    pub symbol_count: usize,
    pub doc_block_count: usize,
    pub documents_edge_count: usize,
    pub doc_source_id: Option<String>,
}

impl<C: Connection> DocxControlPlane<C> {
    /// Ingests C# XML documentation into the store.
    ///
    /// # Errors
    /// Returns `ControlError` if validation fails, parsing fails, or store writes fail.
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

        if let Some(ref assembly_name) = parsed.assembly_name {
            let _ = self
                .upsert_project(ProjectUpsertRequest {
                    project_id: project_id.clone(),
                    name: None,
                    language: Some("csharp".to_string()),
                    root_path: None,
                    description: None,
                    aliases: vec![assembly_name.clone()],
                })
                .await?;
        }

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

    /// Ingests rustdoc JSON documentation into the store.
    ///
    /// # Errors
    /// Returns `ControlError` if validation fails, parsing fails, or store writes fail.
    pub async fn ingest_rustdoc_json(
        &self,
        request: RustdocIngestRequest,
    ) -> Result<RustdocIngestReport, ControlError> {
        let RustdocIngestRequest {
            project_id,
            json,
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

        let mut options = RustdocParseOptions::new(project_id.clone());
        if let Some(ref ingest_id) = ingest_id {
            options = options.with_ingest_id(ingest_id.clone());
        }

        let parsed = RustdocJsonParser::parse_async(json, options).await?;

        if let Some(ref crate_name) = parsed.crate_name {
            let _ = self
                .upsert_project(ProjectUpsertRequest {
                    project_id: project_id.clone(),
                    name: None,
                    language: Some("rust".to_string()),
                    root_path: None,
                    description: None,
                    aliases: vec![crate_name.clone()],
                })
                .await?;
        }

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
                language: Some("rust".to_string()),
                source_kind: Some(SOURCE_KIND_RUSTDOC_JSON.to_string()),
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

        Ok(RustdocIngestReport {
            crate_name: parsed.crate_name,
            symbol_count: stored_symbols.len(),
            doc_block_count: stored_blocks.len(),
            documents_edge_count,
            doc_source_id,
        })
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
