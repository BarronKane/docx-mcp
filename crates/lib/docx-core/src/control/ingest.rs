use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;

use docx_store::models::{DocBlock, DocSource, Ingest, RelationRecord, Symbol};
use docx_store::schema::{
    REL_CONTAINS, REL_DOCUMENTS, REL_IMPLEMENTS, REL_INHERITS, REL_MEMBER_OF, REL_OBSERVED_IN,
    REL_PARAM_TYPE, REL_REFERENCES, REL_RETURNS, REL_SEE_ALSO, SOURCE_KIND_CSHARP_XML,
    SOURCE_KIND_RUSTDOC_JSON, TABLE_DOC_BLOCK, TABLE_DOC_SOURCE, TABLE_SYMBOL,
    make_csharp_symbol_key, make_record_id, make_symbol_key,
};
use serde::{Deserialize, Serialize};
use surrealdb::Connection;
use tokio::fs;

use crate::parsers::{CsharpParseOptions, CsharpXmlParser, RustdocJsonParser, RustdocParseOptions};
use crate::store::StoreError;

use super::metadata::ProjectUpsertRequest;
use super::{ControlError, DocxControlPlane};

/// Input payload for ingesting C# XML documentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsharpIngestRequest {
    pub project_id: String,
    pub xml: Option<String>,
    pub xml_path: Option<String>,
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
    pub json: Option<String>,
    pub json_path: Option<String>,
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
            xml_path,
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

        let xml = resolve_ingest_payload(xml, xml_path, "xml")
            .await
            .map_err(ControlError::Store)?;

        let mut options = CsharpParseOptions::new(project_id.clone());
        if let Some(ref ingest_id) = ingest_id {
            options = options.with_ingest_id(ingest_id.clone());
        }

        let parsed = CsharpXmlParser::parse_async(xml, options).await?;
        let ingest_source_modified_at = source_modified_at.clone();

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

        let stored_symbols = self.store_symbols(parsed.symbols).await?;
        let stored_blocks = self.store.create_doc_blocks(parsed.doc_blocks).await?;
        let doc_source_id = self
            .create_doc_source_if_needed(DocSourceInput {
                project_id: project_id.clone(),
                ingest_id: ingest_id.clone(),
                language: "csharp".to_string(),
                source_kind: SOURCE_KIND_CSHARP_XML.to_string(),
                source_path,
                tool_version,
                source_hash,
                source_modified_at,
                extra: None,
            })
            .await?;
        let documents_edge_count = self
            .persist_relations(
                &stored_symbols,
                &stored_blocks,
                &project_id,
                ingest_id.as_deref(),
                doc_source_id.as_deref(),
                &HashMap::new(),
            )
            .await?;
        let _ = self
            .create_ingest_record(
                &project_id,
                ingest_id.as_deref(),
                ingest_source_modified_at,
                None,
            )
            .await?;

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
            json_path,
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

        let json = resolve_ingest_payload(json, json_path, "json")
            .await
            .map_err(ControlError::Store)?;

        let mut options = RustdocParseOptions::new(project_id.clone());
        if let Some(ref ingest_id) = ingest_id {
            options = options.with_ingest_id(ingest_id.clone());
        }

        let parsed = RustdocJsonParser::parse_async(json, options).await?;
        let ingest_source_modified_at = source_modified_at.clone();

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

        let stored_symbols = self.store_symbols(parsed.symbols).await?;
        let stored_blocks = self.store.create_doc_blocks(parsed.doc_blocks).await?;
        let doc_source_extra = serde_json::json!({
            "format_version": parsed.format_version,
            "includes_private": parsed.includes_private,
        });
        let doc_source_id = self
            .create_doc_source_if_needed(DocSourceInput {
                project_id: project_id.clone(),
                ingest_id: ingest_id.clone(),
                language: "rust".to_string(),
                source_kind: SOURCE_KIND_RUSTDOC_JSON.to_string(),
                source_path,
                tool_version,
                source_hash,
                source_modified_at,
                extra: Some(doc_source_extra),
            })
            .await?;
        let documents_edge_count = self
            .persist_relations(
                &stored_symbols,
                &stored_blocks,
                &project_id,
                ingest_id.as_deref(),
                doc_source_id.as_deref(),
                &parsed.trait_impls,
            )
            .await?;
        let _ = self
            .create_ingest_record(
                &project_id,
                ingest_id.as_deref(),
                ingest_source_modified_at,
                parsed.crate_version.clone(),
            )
            .await?;

        Ok(RustdocIngestReport {
            crate_name: parsed.crate_name,
            symbol_count: stored_symbols.len(),
            doc_block_count: stored_blocks.len(),
            documents_edge_count,
            doc_source_id,
        })
    }

    async fn store_symbols(&self, symbols: Vec<Symbol>) -> Result<Vec<Symbol>, ControlError> {
        let mut stored = Vec::new();
        for symbol in dedupe_symbols(symbols) {
            stored.push(self.store.upsert_symbol(symbol).await?);
        }
        Ok(stored)
    }

    async fn create_doc_source_if_needed(
        &self,
        input: DocSourceInput,
    ) -> Result<Option<String>, ControlError> {
        let has_source = input.source_path.is_some()
            || input.tool_version.is_some()
            || input.source_hash.is_some()
            || input.source_modified_at.is_some()
            || input.extra.is_some();
        if !has_source {
            return Ok(None);
        }

        let source = DocSource {
            id: None,
            project_id: input.project_id,
            ingest_id: input.ingest_id,
            language: Some(input.language),
            source_kind: Some(input.source_kind),
            path: input.source_path,
            tool_version: input.tool_version,
            hash: input.source_hash,
            source_modified_at: input.source_modified_at,
            extra: input.extra,
        };
        let created = self.store.create_doc_source(source).await?;
        Ok(created.id)
    }

    async fn create_ingest_record(
        &self,
        project_id: &str,
        ingest_id: Option<&str>,
        source_modified_at: Option<String>,
        project_version: Option<String>,
    ) -> Result<Option<String>, ControlError> {
        let ingest = Ingest {
            id: ingest_id.map(str::to_string),
            project_id: project_id.to_string(),
            git_commit: None,
            git_branch: None,
            git_tag: None,
            project_version,
            source_modified_at,
            ingested_at: Some(chrono::Utc::now().to_rfc3339()),
            extra: None,
        };
        let created = self.store.create_ingest(ingest).await?;
        Ok(created.id)
    }

    async fn persist_relations(
        &self,
        stored_symbols: &[Symbol],
        stored_blocks: &[DocBlock],
        project_id: &str,
        ingest_id: Option<&str>,
        doc_source_id: Option<&str>,
        trait_impls: &HashMap<String, Vec<String>>,
    ) -> Result<usize, ControlError> {
        let documents = build_documents_edges(stored_symbols, stored_blocks, project_id, ingest_id);
        let documents_edge_count = documents.len();
        if !documents.is_empty() {
            let _ = self
                .store
                .create_relations(REL_DOCUMENTS, documents)
                .await?;
        }

        let relations = build_symbol_relations(stored_symbols, project_id, ingest_id, trait_impls);
        if !relations.is_empty() {
            let _ = self
                .store
                .create_relations(REL_MEMBER_OF, relations.member_of)
                .await?;
            let _ = self
                .store
                .create_relations(REL_CONTAINS, relations.contains)
                .await?;
            let _ = self
                .store
                .create_relations(REL_RETURNS, relations.returns)
                .await?;
            let _ = self
                .store
                .create_relations(REL_PARAM_TYPE, relations.param_types)
                .await?;
            if !relations.implements.is_empty() {
                let _ = self
                    .store
                    .create_relations(REL_IMPLEMENTS, relations.implements)
                    .await?;
            }
        }

        let doc_relations =
            build_doc_block_relations(stored_symbols, stored_blocks, project_id, ingest_id);
        if !doc_relations.is_empty() {
            let _ = self
                .store
                .create_relations(REL_SEE_ALSO, doc_relations.see_also)
                .await?;
            let _ = self
                .store
                .create_relations(REL_INHERITS, doc_relations.inherits)
                .await?;
            let _ = self
                .store
                .create_relations(REL_REFERENCES, doc_relations.references)
                .await?;
        }

        if let Some(doc_source_id) = doc_source_id {
            let observed_in =
                build_observed_in_edges(stored_symbols, project_id, ingest_id, doc_source_id);
            if !observed_in.is_empty() {
                let _ = self
                    .store
                    .create_relations(REL_OBSERVED_IN, observed_in)
                    .await?;
            }
        }

        Ok(documents_edge_count)
    }
}

async fn resolve_ingest_payload(
    raw: Option<String>,
    path: Option<String>,
    field: &str,
) -> Result<String, StoreError> {
    if let Some(value) = normalize_payload(raw) {
        return Ok(strip_bom(&value));
    }
    if let Some(path) = normalize_payload(path) {
        let contents = fs::read_to_string(&path).await.map_err(|err| {
            let mut message = format!("failed to read {field}_path '{path}': {err}");
            if err.kind() == ErrorKind::NotFound {
                message.push_str(
                    "; file not found on server host. If running in Docker, mount the file into the container or send raw contents instead.",
                );
            }
            StoreError::InvalidInput(message)
        })?;
        return Ok(strip_bom(&contents));
    }
    Err(StoreError::InvalidInput(format!(
        "{field} is required (provide {field} or {field}_path)"
    )))
}

fn normalize_payload(value: Option<String>) -> Option<String> {
    value.and_then(|payload| {
        let trimmed = payload.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(payload)
        }
    })
}

fn strip_bom(value: &str) -> String {
    value.strip_prefix('\u{feff}').unwrap_or(value).to_string()
}

fn dedupe_symbols(symbols: Vec<Symbol>) -> Vec<Symbol> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        if seen.insert(symbol.symbol_key.clone()) {
            deduped.push(symbol);
        }
    }
    deduped
}

struct DocSourceInput {
    project_id: String,
    ingest_id: Option<String>,
    language: String,
    source_kind: String,
    source_path: Option<String>,
    tool_version: Option<String>,
    source_hash: Option<String>,
    source_modified_at: Option<String>,
    extra: Option<serde_json::Value>,
}

/// Builds `documents` relation edges between doc blocks and symbols.
fn build_documents_edges(
    symbols: &[Symbol],
    blocks: &[DocBlock],
    project_id: &str,
    ingest_id: Option<&str>,
) -> Vec<RelationRecord> {
    let mut symbol_map = HashMap::new();
    for symbol in symbols {
        if let Some(id) = symbol.id.as_ref() {
            let record_id = make_record_id(TABLE_SYMBOL, id);
            symbol_map.insert(symbol.symbol_key.as_str(), record_id);
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
        let block_record_id = make_record_id(TABLE_DOC_BLOCK, block_id);
        relations.push(RelationRecord {
            id: None,
            in_id: block_record_id,
            out_id: symbol_id.clone(),
            project_id: project_id.to_string(),
            ingest_id: ingest_id.map(str::to_string),
            kind: None,
            extra: None,
        });
    }
    relations
}

/// Builds `observed_in` relation edges between symbols and the ingested doc source.
fn build_observed_in_edges(
    symbols: &[Symbol],
    project_id: &str,
    ingest_id: Option<&str>,
    doc_source_id: &str,
) -> Vec<RelationRecord> {
    let doc_source_record = make_record_id(TABLE_DOC_SOURCE, doc_source_id);
    symbols
        .iter()
        .filter_map(|symbol| {
            symbol.id.as_ref().map(|symbol_id| RelationRecord {
                id: None,
                in_id: make_record_id(TABLE_SYMBOL, symbol_id),
                out_id: doc_source_record.clone(),
                project_id: project_id.to_string(),
                ingest_id: ingest_id.map(str::to_string),
                kind: Some("doc_source".to_string()),
                extra: None,
            })
        })
        .collect()
}

/// Bundles relation edges derived from symbol metadata.
#[derive(Default)]
struct SymbolRelations {
    member_of: Vec<RelationRecord>,
    contains: Vec<RelationRecord>,
    returns: Vec<RelationRecord>,
    param_types: Vec<RelationRecord>,
    implements: Vec<RelationRecord>,
}

impl SymbolRelations {
    /// Returns true when all relation collections are empty.
    const fn is_empty(&self) -> bool {
        self.member_of.is_empty()
            && self.contains.is_empty()
            && self.returns.is_empty()
            && self.param_types.is_empty()
            && self.implements.is_empty()
    }
}

/// Builds relation edges for symbol membership, containment, type references, and trait impls.
fn build_symbol_relations(
    symbols: &[Symbol],
    project_id: &str,
    ingest_id: Option<&str>,
    trait_impls: &HashMap<String, Vec<String>>,
) -> SymbolRelations {
    let mut relations = SymbolRelations::default();
    let mut symbol_by_qualified = HashMap::new();
    let mut symbol_by_key = HashMap::new();

    for symbol in symbols {
        if let (Some(id), Some(qualified_name)) =
            (symbol.id.as_ref(), symbol.qualified_name.as_ref())
        {
            symbol_by_qualified.insert(qualified_name.as_str(), id.as_str());
        }
        if let Some(id) = symbol.id.as_ref() {
            symbol_by_key.insert(symbol.symbol_key.as_str(), id.as_str());
        }
    }

    for symbol in symbols {
        let Some(symbol_id) = symbol.id.as_ref() else {
            continue;
        };
        let symbol_record = make_record_id(TABLE_SYMBOL, symbol_id);
        let ingest_id = ingest_id.map(str::to_string);

        if let Some(parent) = symbol
            .qualified_name
            .as_ref()
            .and_then(|qualified| qualified.rsplit_once("::").map(|pair| pair.0.to_string()))
            .and_then(|parent| symbol_by_qualified.get(parent.as_str()).copied())
        {
            let parent_record = make_record_id(TABLE_SYMBOL, parent);
            relations.member_of.push(RelationRecord {
                id: None,
                in_id: symbol_record.clone(),
                out_id: parent_record.clone(),
                project_id: project_id.to_string(),
                ingest_id: ingest_id.clone(),
                kind: None,
                extra: None,
            });
            relations.contains.push(RelationRecord {
                id: None,
                in_id: parent_record,
                out_id: symbol_record.clone(),
                project_id: project_id.to_string(),
                ingest_id: ingest_id.clone(),
                kind: None,
                extra: None,
            });
        }

        if let Some(return_key) = symbol
            .return_type
            .as_ref()
            .and_then(|ty| ty.symbol_key.as_ref())
            .and_then(|key| symbol_by_key.get(key.as_str()).copied())
        {
            relations.returns.push(RelationRecord {
                id: None,
                in_id: symbol_record.clone(),
                out_id: make_record_id(TABLE_SYMBOL, return_key),
                project_id: project_id.to_string(),
                ingest_id: ingest_id.clone(),
                kind: None,
                extra: None,
            });
        }

        for param in &symbol.params {
            let Some(param_key) = param
                .type_ref
                .as_ref()
                .and_then(|ty| ty.symbol_key.as_ref())
                .and_then(|key| symbol_by_key.get(key.as_str()).copied())
            else {
                continue;
            };
            relations.param_types.push(RelationRecord {
                id: None,
                in_id: symbol_record.clone(),
                out_id: make_record_id(TABLE_SYMBOL, param_key),
                project_id: project_id.to_string(),
                ingest_id: ingest_id.clone(),
                kind: Some(param.name.clone()),
                extra: None,
            });
        }

        // Build implements edges from trait_impls map
        if let Some(qualified_name) = symbol.qualified_name.as_ref()
            && let Some(trait_paths) = trait_impls.get(qualified_name.as_str())
        {
            for trait_path in trait_paths {
                let trait_key = make_symbol_key("rust", project_id, trait_path);
                if let Some(trait_id) = symbol_by_key.get(trait_key.as_str()).copied() {
                    relations.implements.push(RelationRecord {
                        id: None,
                        in_id: symbol_record.clone(),
                        out_id: make_record_id(TABLE_SYMBOL, trait_id),
                        project_id: project_id.to_string(),
                        ingest_id: ingest_id.clone(),
                        kind: Some("trait_impl".to_string()),
                        extra: None,
                    });
                }
            }
        }
    }

    relations
}

/// Bundles relation edges derived from documentation metadata.
#[derive(Default)]
struct DocBlockRelations {
    see_also: Vec<RelationRecord>,
    inherits: Vec<RelationRecord>,
    references: Vec<RelationRecord>,
}

impl DocBlockRelations {
    /// Returns true when all relation collections are empty.
    const fn is_empty(&self) -> bool {
        self.see_also.is_empty() && self.inherits.is_empty() && self.references.is_empty()
    }
}

/// Builds relation edges for `see also`, inheritance, and reference metadata on doc blocks.
fn build_doc_block_relations(
    symbols: &[Symbol],
    blocks: &[DocBlock],
    project_id: &str,
    ingest_id: Option<&str>,
) -> DocBlockRelations {
    let mut relations = DocBlockRelations::default();
    let mut symbol_by_key = HashMap::new();
    for symbol in symbols {
        if let Some(id) = symbol.id.as_ref() {
            symbol_by_key.insert(symbol.symbol_key.as_str(), id.as_str());
        }
    }

    for block in blocks {
        let Some(symbol_key) = block.symbol_key.as_ref() else {
            continue;
        };
        let Some(symbol_id) = symbol_by_key.get(symbol_key.as_str()).copied() else {
            continue;
        };
        let symbol_record = make_record_id(TABLE_SYMBOL, symbol_id);
        let ingest_id = ingest_id.map(str::to_string);
        let language = block.language.as_deref();

        for link in &block.see_also {
            if let Some(target_id) =
                resolve_symbol_reference(&link.target, language, project_id, &symbol_by_key)
            {
                relations.see_also.push(RelationRecord {
                    id: None,
                    in_id: symbol_record.clone(),
                    out_id: make_record_id(TABLE_SYMBOL, target_id),
                    project_id: project_id.to_string(),
                    ingest_id: ingest_id.clone(),
                    kind: link.target_kind.clone(),
                    extra: None,
                });
            }
        }

        if let Some(inherit) = block.inherit_doc.as_ref() {
            let target = inherit.cref.as_deref().or(inherit.path.as_deref());
            if let Some(target) = target
                && let Some(target_id) =
                    resolve_symbol_reference(target, language, project_id, &symbol_by_key)
            {
                relations.inherits.push(RelationRecord {
                    id: None,
                    in_id: symbol_record.clone(),
                    out_id: make_record_id(TABLE_SYMBOL, target_id),
                    project_id: project_id.to_string(),
                    ingest_id: ingest_id.clone(),
                    kind: Some("inheritdoc".to_string()),
                    extra: None,
                });
            }
        }

        for exception in &block.exceptions {
            let Some(target_id) = exception
                .type_ref
                .as_ref()
                .and_then(|ty| ty.symbol_key.as_ref())
                .and_then(|key| symbol_by_key.get(key.as_str()).copied())
            else {
                continue;
            };
            relations.references.push(RelationRecord {
                id: None,
                in_id: symbol_record.clone(),
                out_id: make_record_id(TABLE_SYMBOL, target_id),
                project_id: project_id.to_string(),
                ingest_id: ingest_id.clone(),
                kind: Some("exception".to_string()),
                extra: None,
            });
        }
    }

    relations
}

fn resolve_symbol_reference<'a>(
    target: &str,
    language: Option<&str>,
    project_id: &str,
    symbol_by_key: &'a HashMap<&'a str, &'a str>,
) -> Option<&'a str> {
    if let Some(id) = symbol_by_key.get(target).copied() {
        return Some(id);
    }
    match language {
        Some("csharp") => {
            let key = make_csharp_symbol_key(project_id, target);
            symbol_by_key.get(key.as_str()).copied()
        }
        Some("rust") => {
            let key = make_symbol_key("rust", project_id, target);
            symbol_by_key.get(key.as_str()).copied()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docx_store::models::{DocException, DocInherit, SeeAlso, TypeRef};

    fn build_symbol(project_id: &str, id: &str, key: &str) -> Symbol {
        Symbol {
            id: Some(id.to_string()),
            project_id: project_id.to_string(),
            language: Some("csharp".to_string()),
            symbol_key: key.to_string(),
            kind: None,
            name: None,
            qualified_name: None,
            display_name: None,
            signature: None,
            signature_hash: None,
            visibility: None,
            is_static: None,
            is_async: None,
            is_const: None,
            is_deprecated: None,
            since: None,
            stability: None,
            source_path: None,
            line: None,
            col: None,
            return_type: None,
            params: Vec::new(),
            type_params: Vec::new(),
            attributes: Vec::new(),
            source_ids: Vec::new(),
            doc_summary: None,
            extra: None,
        }
    }

    fn build_doc_block(project_id: &str, symbol_key: &str) -> DocBlock {
        DocBlock {
            id: Some("block-1".to_string()),
            project_id: project_id.to_string(),
            ingest_id: None,
            symbol_key: Some(symbol_key.to_string()),
            language: Some("csharp".to_string()),
            source_kind: Some(SOURCE_KIND_CSHARP_XML.to_string()),
            doc_hash: None,
            summary: None,
            remarks: None,
            returns: None,
            value: None,
            params: Vec::new(),
            type_params: Vec::new(),
            exceptions: Vec::new(),
            examples: Vec::new(),
            notes: Vec::new(),
            warnings: Vec::new(),
            safety: None,
            panics: None,
            errors: None,
            see_also: Vec::new(),
            deprecated: None,
            inherit_doc: None,
            sections: Vec::new(),
            raw: None,
            extra: None,
        }
    }

    #[test]
    fn build_observed_in_edges_links_symbols_to_doc_source() {
        let symbols = vec![
            build_symbol("docx", "foo", "csharp|docx|T:Foo"),
            build_symbol("docx", "bar", "csharp|docx|T:Bar"),
        ];

        let edges = build_observed_in_edges(&symbols, "docx", Some("ing-1"), "source-1");
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].in_id, make_record_id(TABLE_SYMBOL, "foo"));
        assert_eq!(
            edges[0].out_id,
            make_record_id(TABLE_DOC_SOURCE, "source-1")
        );
        assert_eq!(edges[0].ingest_id.as_deref(), Some("ing-1"));
        assert_eq!(edges[0].kind.as_deref(), Some("doc_source"));
    }

    #[test]
    fn build_doc_block_relations_extracts_csharp_references() {
        let project_id = "docx";
        let foo_key = make_csharp_symbol_key(project_id, "T:Foo");
        let bar_key = make_csharp_symbol_key(project_id, "T:Bar");

        let symbols = vec![
            build_symbol(project_id, "foo", &foo_key),
            build_symbol(project_id, "bar", &bar_key),
        ];

        let mut block = build_doc_block(project_id, &foo_key);
        block.see_also.push(SeeAlso {
            label: Some("Bar".to_string()),
            target: "T:Bar".to_string(),
            target_kind: Some("cref".to_string()),
        });
        block.inherit_doc = Some(DocInherit {
            cref: Some("T:Bar".to_string()),
            path: None,
        });
        block.exceptions.push(DocException {
            type_ref: Some(TypeRef {
                display: Some("Bar".to_string()),
                canonical: Some("Bar".to_string()),
                language: Some("csharp".to_string()),
                symbol_key: Some(bar_key),
                generics: Vec::new(),
                modifiers: Vec::new(),
            }),
            description: None,
        });

        let relations = build_doc_block_relations(&symbols, &[block], project_id, None);

        assert_eq!(relations.see_also.len(), 1);
        assert_eq!(relations.inherits.len(), 1);
        assert_eq!(relations.references.len(), 1);

        let target_record = make_record_id(TABLE_SYMBOL, "bar");
        assert_eq!(relations.see_also[0].out_id, target_record);
        assert_eq!(relations.see_also[0].kind.as_deref(), Some("cref"));
        assert_eq!(relations.inherits[0].kind.as_deref(), Some("inheritdoc"));
        assert_eq!(relations.references[0].kind.as_deref(), Some("exception"));
    }

    #[test]
    fn dedupe_symbols_keeps_first_symbol_per_key() {
        let mut first = build_symbol("docx", "first", "csharp|docx|T:Foo");
        first.name = Some("first".to_string());
        let mut duplicate = build_symbol("docx", "second", "csharp|docx|T:Foo");
        duplicate.name = Some("second".to_string());
        let other = build_symbol("docx", "third", "csharp|docx|T:Bar");

        let deduped = dedupe_symbols(vec![first.clone(), duplicate, other.clone()]);

        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].symbol_key, first.symbol_key);
        assert_eq!(deduped[0].name.as_deref(), Some("first"));
        assert_eq!(deduped[1].symbol_key, other.symbol_key);
    }
}
