use docx_store::models::{DocBlock, DocSource, RelationRecord, Symbol};
use surrealdb::Connection;

use super::{ControlError, DocxControlPlane};

impl<C: Connection> DocxControlPlane<C> {
    /// Fetches a symbol by project and key.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
    pub async fn get_symbol(
        &self,
        project_id: &str,
        symbol_key: &str,
    ) -> Result<Option<Symbol>, ControlError> {
        let record = self
            .store
            .get_symbol_by_project(project_id, symbol_key)
            .await?;
        if record.is_some() {
            return Ok(record);
        }
        Ok(self.store.get_symbol(symbol_key).await?)
    }

    /// Lists document blocks for a symbol, optionally scoping by ingest id.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
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

    /// Searches symbols by name.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
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

    /// Searches document blocks by text.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
    pub async fn search_doc_blocks(
        &self,
        project_id: &str,
        text: &str,
        limit: usize,
    ) -> Result<Vec<DocBlock>, ControlError> {
        Ok(self
            .store
            .search_doc_blocks(project_id, text, limit)
            .await?)
    }

    /// Lists distinct symbol kinds for a project.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
    pub async fn list_symbol_kinds(&self, project_id: &str) -> Result<Vec<String>, ControlError> {
        Ok(self.store.list_symbol_kinds(project_id).await?)
    }

    /// Lists members by scope prefix or glob pattern.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
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

    /// Fetches adjacency information for a symbol, including relations and related symbols.
    ///
    /// Uses a single multi-statement query for all relation types to minimize DB round trips.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
    pub async fn get_symbol_adjacency(
        &self,
        project_id: &str,
        symbol_key: &str,
        limit: usize,
    ) -> Result<SymbolAdjacency, ControlError> {
        let limit = limit.max(1);
        let symbol = self.get_symbol(project_id, symbol_key).await?;
        let Some(symbol) = symbol else {
            return Ok(SymbolAdjacency::default());
        };
        let doc_blocks = self.list_doc_blocks(project_id, symbol_key, None).await?;
        let mut ingest_ids = doc_blocks
            .iter()
            .filter_map(|block| block.ingest_id.clone())
            .collect::<Vec<_>>();
        ingest_ids.sort();
        ingest_ids.dedup();
        let doc_sources = self.store.list_doc_sources(project_id, &ingest_ids).await?;
        let symbol_id = symbol
            .id
            .clone()
            .unwrap_or_else(|| symbol.symbol_key.clone());

        let adj = self
            .store
            .fetch_symbol_adjacency(&symbol_id, project_id, limit)
            .await?;

        let mut related_keys = std::collections::HashSet::new();
        for relation in adj
            .member_of
            .iter()
            .chain(adj.contains.iter())
            .chain(adj.returns.iter())
            .chain(adj.param_types.iter())
            .chain(adj.see_also.iter())
            .chain(adj.inherits.iter())
            .chain(adj.references.iter())
            .chain(adj.observed_in.iter())
        {
            if let Some(key) = record_id_to_symbol_key(&relation.in_id) {
                related_keys.insert(key.to_string());
            }
            if let Some(key) = record_id_to_symbol_key(&relation.out_id) {
                related_keys.insert(key.to_string());
            }
        }

        let related_keys: Vec<String> = related_keys.into_iter().collect();
        let related_futs: Vec<_> = related_keys
            .iter()
            .map(|key| self.get_symbol(project_id, key))
            .collect();
        let related_results = futures::future::join_all(related_futs).await;
        let mut related_symbols: Vec<Symbol> = related_results
            .into_iter()
            .filter_map(|r| r.ok().flatten())
            .collect();
        related_symbols.sort_by(|left, right| left.symbol_key.cmp(&right.symbol_key));
        related_symbols.dedup_by(|left, right| left.symbol_key == right.symbol_key);

        Ok(SymbolAdjacency {
            symbol: Some(symbol),
            doc_blocks,
            doc_sources,
            member_of: adj.member_of,
            contains: adj.contains,
            returns: adj.returns,
            param_types: adj.param_types,
            see_also: adj.see_also,
            inherits: adj.inherits,
            references: adj.references,
            observed_in: adj.observed_in,
            related_symbols,
        })
    }
}

/// Relation graph data for a symbol.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SymbolAdjacency {
    pub symbol: Option<Symbol>,
    pub doc_blocks: Vec<DocBlock>,
    pub doc_sources: Vec<DocSource>,
    pub member_of: Vec<RelationRecord>,
    pub contains: Vec<RelationRecord>,
    pub returns: Vec<RelationRecord>,
    pub param_types: Vec<RelationRecord>,
    pub see_also: Vec<RelationRecord>,
    pub inherits: Vec<RelationRecord>,
    pub references: Vec<RelationRecord>,
    pub observed_in: Vec<RelationRecord>,
    pub related_symbols: Vec<Symbol>,
}

/// Extracts the symbol key from a table-qualified record id.
fn record_id_to_symbol_key(record_id: &str) -> Option<&str> {
    record_id.strip_prefix("symbol:")
}
