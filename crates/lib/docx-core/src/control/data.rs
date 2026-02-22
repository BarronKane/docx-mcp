use std::collections::{BTreeMap, BTreeSet, HashSet};

use docx_store::models::{DocBlock, DocSource, RelationRecord, Symbol};
use docx_store::schema::{
    REL_CONTAINS, REL_INHERITS, REL_MEMBER_OF, REL_OBSERVED_IN, REL_PARAM_TYPE, REL_REFERENCES,
    REL_RETURNS, REL_SEE_ALSO, TABLE_DOC_BLOCK, TABLE_DOC_SOURCE, TABLE_SYMBOL,
};
use surrealdb::Connection;

use crate::store::StoreError;

use super::{ControlError, DocxControlPlane};

const ADVANCED_SEARCH_MIN_FILTERS: usize = 1;

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
        Ok(self
            .store
            .get_symbol_by_project(project_id, symbol_key)
            .await?)
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

    /// Searches symbols with optional exact/fuzzy filters.
    ///
    /// # Errors
    /// Returns `ControlError` if no filters are provided or the store query fails.
    pub async fn search_symbols_advanced(
        &self,
        project_id: &str,
        request: SearchSymbolsAdvancedRequest,
        limit: usize,
    ) -> Result<SearchSymbolsAdvancedResult, ControlError> {
        let normalized = request.normalized();
        if normalized.active_filter_count() < ADVANCED_SEARCH_MIN_FILTERS {
            return Err(ControlError::Store(StoreError::InvalidInput(
                "at least one search filter is required".to_string(),
            )));
        }

        let symbols = self
            .store
            .search_symbols_advanced(
                project_id,
                normalized.name.as_deref(),
                normalized.qualified_name.as_deref(),
                normalized.symbol_key.as_deref(),
                normalized.signature.as_deref(),
                limit,
            )
            .await?;
        let total_returned = symbols.len();

        Ok(SearchSymbolsAdvancedResult {
            symbols,
            total_returned,
            applied_filters: normalized,
        })
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

    /// Audits high-level documentation graph completeness for a project.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
    pub async fn audit_project_completeness(
        &self,
        project_id: &str,
    ) -> Result<ProjectCompletenessAudit, ControlError> {
        let symbol_count = self
            .store
            .count_rows_for_project(TABLE_SYMBOL, project_id)
            .await?;
        let doc_block_count = self
            .store
            .count_rows_for_project(TABLE_DOC_BLOCK, project_id)
            .await?;
        let doc_source_count = self
            .store
            .count_rows_for_project(TABLE_DOC_SOURCE, project_id)
            .await?;

        let symbols_missing_source_path_count = self
            .store
            .count_symbols_missing_field(project_id, "source_path")
            .await?;
        let symbols_missing_line_count = self
            .store
            .count_symbols_missing_field(project_id, "line")
            .await?;
        let symbols_missing_col_count = self
            .store
            .count_symbols_missing_field(project_id, "col")
            .await?;

        let doc_block_symbol_keys = self.store.list_doc_block_symbol_keys(project_id).await?;
        let symbols_with_doc_blocks_count = doc_block_symbol_keys
            .into_iter()
            .collect::<HashSet<_>>()
            .len();

        let observed_in_symbols = self.store.list_observed_in_symbol_refs(project_id).await?;
        let symbols_with_observed_in_count = observed_in_symbols
            .into_iter()
            .collect::<HashSet<_>>()
            .len();

        let relation_edge_counts = relation_names()
            .into_iter()
            .map(|relation| async move {
                let count = self
                    .store
                    .count_rows_for_project(relation, project_id)
                    .await?;
                Ok::<RelationEdgeCount, ControlError>(RelationEdgeCount {
                    relation: relation.to_string(),
                    count,
                })
            })
            .collect::<Vec<_>>();

        let mut relation_edge_counts = futures::future::try_join_all(relation_edge_counts).await?;
        relation_edge_counts.sort_by(|left, right| left.relation.cmp(&right.relation));

        let relation_counts = relation_edge_counts
            .iter()
            .map(|entry| (entry.relation.clone(), entry.count))
            .collect::<BTreeMap<_, _>>();

        Ok(ProjectCompletenessAudit {
            project_id: project_id.to_string(),
            symbol_count,
            doc_block_count,
            doc_source_count,
            symbols_missing_source_path_count,
            symbols_missing_line_count,
            symbols_missing_col_count,
            symbols_with_doc_blocks_count,
            symbols_with_observed_in_count,
            relation_counts,
            relation_edge_counts,
        })
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
        let symbol_id = symbol
            .id
            .clone()
            .unwrap_or_else(|| symbol.symbol_key.clone());

        let adj = self
            .store
            .fetch_symbol_adjacency(&symbol_id, project_id, limit)
            .await?;

        let doc_sources_from_doc_blocks =
            self.store.list_doc_sources(project_id, &ingest_ids).await?;
        let observed_doc_source_ids = adj
            .observed_in
            .iter()
            .filter_map(|edge| record_id_to_doc_source_id(&edge.out_id))
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let doc_sources_from_observed_in = self
            .store
            .list_doc_sources_by_ids(project_id, &observed_doc_source_ids)
            .await?;
        let hydration_summary = DocSourceHydrationSummary {
            from_doc_blocks: doc_sources_from_doc_blocks.len(),
            from_observed_in: doc_sources_from_observed_in.len(),
            deduped_total: 0,
        };
        let (doc_sources, hydration_summary) = merge_doc_sources(
            doc_sources_from_doc_blocks,
            doc_sources_from_observed_in,
            hydration_summary,
        );

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
            hydration_summary,
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
    pub hydration_summary: DocSourceHydrationSummary,
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

/// Summary of where adjacency `doc_sources` were hydrated from.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DocSourceHydrationSummary {
    pub from_doc_blocks: usize,
    pub from_observed_in: usize,
    pub deduped_total: usize,
}

/// Input filters for advanced symbol search.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SearchSymbolsAdvancedRequest {
    pub name: Option<String>,
    pub qualified_name: Option<String>,
    pub symbol_key: Option<String>,
    pub signature: Option<String>,
}

impl SearchSymbolsAdvancedRequest {
    #[must_use]
    pub fn normalized(self) -> Self {
        Self {
            name: normalize_optional(self.name),
            qualified_name: normalize_optional(self.qualified_name),
            symbol_key: normalize_optional(self.symbol_key),
            signature: normalize_optional(self.signature),
        }
    }

    #[must_use]
    pub fn active_filter_count(&self) -> usize {
        [
            self.name.as_ref(),
            self.qualified_name.as_ref(),
            self.symbol_key.as_ref(),
            self.signature.as_ref(),
        ]
        .iter()
        .filter(|value| value.is_some())
        .count()
    }
}

/// Output payload for advanced symbol search.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SearchSymbolsAdvancedResult {
    pub symbols: Vec<Symbol>,
    pub total_returned: usize,
    pub applied_filters: SearchSymbolsAdvancedRequest,
}

/// Relation edge counts used in project completeness audits.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RelationEdgeCount {
    pub relation: String,
    pub count: usize,
}

/// Project-level completeness audit report.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ProjectCompletenessAudit {
    pub project_id: String,
    pub symbol_count: usize,
    pub doc_block_count: usize,
    pub doc_source_count: usize,
    pub symbols_missing_source_path_count: usize,
    pub symbols_missing_line_count: usize,
    pub symbols_missing_col_count: usize,
    pub symbols_with_doc_blocks_count: usize,
    pub symbols_with_observed_in_count: usize,
    pub relation_counts: BTreeMap<String, usize>,
    pub relation_edge_counts: Vec<RelationEdgeCount>,
}

/// Extracts the symbol key from a table-qualified record id.
fn record_id_to_symbol_key(record_id: &str) -> Option<&str> {
    record_id.strip_prefix("symbol:")
}

/// Extracts a doc-source id from a table-qualified record id.
fn record_id_to_doc_source_id(record_id: &str) -> Option<&str> {
    record_id.strip_prefix("doc_source:")
}

fn merge_doc_sources(
    from_doc_blocks: Vec<DocSource>,
    from_observed_in: Vec<DocSource>,
    mut summary: DocSourceHydrationSummary,
) -> (Vec<DocSource>, DocSourceHydrationSummary) {
    let mut all = from_doc_blocks;
    all.extend(from_observed_in);

    let mut seen = HashSet::new();
    all.retain(|source| {
        let key = source
            .id
            .clone()
            .unwrap_or_else(|| format!("missing:{}", source.project_id));
        seen.insert(key)
    });
    all.sort_by(|left, right| {
        let left_key = left.id.as_deref().unwrap_or_default();
        let right_key = right.id.as_deref().unwrap_or_default();
        left_key.cmp(right_key)
    });
    summary.deduped_total = all.len();
    (all, summary)
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|inner| {
        let trimmed = inner.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn relation_names() -> Vec<&'static str> {
    vec![
        REL_MEMBER_OF,
        REL_CONTAINS,
        REL_RETURNS,
        REL_PARAM_TYPE,
        REL_SEE_ALSO,
        REL_INHERITS,
        REL_REFERENCES,
        REL_OBSERVED_IN,
    ]
}
