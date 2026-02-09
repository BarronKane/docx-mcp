use docx_store::models::{DocBlock, DocSource, RelationRecord, Symbol};
use docx_store::schema::{
    REL_CONTAINS,
    REL_INHERITS,
    REL_MEMBER_OF,
    REL_PARAM_TYPE,
    REL_REFERENCES,
    REL_RETURNS,
    REL_SEE_ALSO,
};
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
        Ok(self.store.search_doc_blocks(project_id, text, limit).await?)
    }

    /// Lists distinct symbol kinds for a project.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
    pub async fn list_symbol_kinds(
        &self,
        project_id: &str,
    ) -> Result<Vec<String>, ControlError> {
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
        let doc_blocks = self
            .list_doc_blocks(project_id, symbol_key, None)
            .await?;
        let mut ingest_ids = doc_blocks
            .iter()
            .filter_map(|block| block.ingest_id.clone())
            .collect::<Vec<_>>();
        ingest_ids.sort();
        ingest_ids.dedup();
        let doc_sources = self.store.list_doc_sources(project_id, &ingest_ids).await?;
        let symbol_id = symbol.id.clone().unwrap_or_else(|| symbol.symbol_key.clone());

        let member_of = self
            .list_relations(REL_MEMBER_OF, project_id, &symbol_id, limit)
            .await?;
        let contains = self
            .list_relations(REL_CONTAINS, project_id, &symbol_id, limit)
            .await?;
        let returns = self
            .list_relations(REL_RETURNS, project_id, &symbol_id, limit)
            .await?;
        let param_types = self
            .list_relations(REL_PARAM_TYPE, project_id, &symbol_id, limit)
            .await?;
        let see_also = self
            .list_relations(REL_SEE_ALSO, project_id, &symbol_id, limit)
            .await?;
        let inherits = self
            .list_relations(REL_INHERITS, project_id, &symbol_id, limit)
            .await?;
        let references = self
            .list_relations(REL_REFERENCES, project_id, &symbol_id, limit)
            .await?;

        let mut related_symbols = Vec::new();
        for relation in member_of
            .iter()
            .chain(contains.iter())
            .chain(returns.iter())
            .chain(param_types.iter())
            .chain(see_also.iter())
            .chain(inherits.iter())
            .chain(references.iter())
        {
            if let Some(symbol_key) = record_id_to_symbol_key(&relation.in_id)
                && let Some(found) = self.get_symbol(project_id, symbol_key).await?
            {
                related_symbols.push(found);
            }
            if let Some(symbol_key) = record_id_to_symbol_key(&relation.out_id)
                && let Some(found) = self.get_symbol(project_id, symbol_key).await?
            {
                related_symbols.push(found);
            }
        }

        related_symbols.sort_by(|left, right| left.symbol_key.cmp(&right.symbol_key));
        related_symbols.dedup_by(|left, right| left.symbol_key == right.symbol_key);

        Ok(SymbolAdjacency {
            symbol: Some(symbol),
            doc_blocks,
            doc_sources,
            member_of,
            contains,
            returns,
            param_types,
            see_also,
            inherits,
            references,
            related_symbols,
        })
    }

    async fn list_relations(
        &self,
        table: &str,
        project_id: &str,
        symbol_id: &str,
        limit: usize,
    ) -> Result<Vec<RelationRecord>, ControlError> {
        let outgoing = self
            .store
            .list_relations_from_symbol(table, project_id, symbol_id, limit)
            .await?;
        let incoming = self
            .store
            .list_relations_to_symbol(table, project_id, symbol_id, limit)
            .await?;
        Ok(merge_relations(outgoing, incoming))
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
    pub related_symbols: Vec<Symbol>,
}

/// Extracts the symbol key from a table-qualified record id.
fn record_id_to_symbol_key(record_id: &str) -> Option<&str> {
    record_id.strip_prefix("symbol:")
}

/// Merges relation records while de-duplicating by record identity.
fn merge_relations(
    mut left: Vec<RelationRecord>,
    right: Vec<RelationRecord>,
) -> Vec<RelationRecord> {
    let mut seen = std::collections::HashSet::new();
    for relation in &left {
        seen.insert(relation_key(relation));
    }
    for relation in right {
        let key = relation_key(&relation);
        if seen.insert(key) {
            left.push(relation);
        }
    }
    left
}

/// Creates a deduplication key for relation records.
fn relation_key(relation: &RelationRecord) -> (String, String, Option<String>) {
    (
        relation.in_id.clone(),
        relation.out_id.clone(),
        relation.kind.clone(),
    )
}
