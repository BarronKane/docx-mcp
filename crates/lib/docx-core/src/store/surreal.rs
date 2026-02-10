use std::{error::Error, fmt, str::FromStr, sync::Arc};

use docx_store::models::{
    DocBlock,
    DocChunk,
    DocSource,
    Ingest,
    Project,
    RelationRecord,
    Symbol,
};
use docx_store::schema::{
    TABLE_DOC_BLOCK,
    TABLE_DOC_SOURCE,
    TABLE_INGEST,
    TABLE_PROJECT,
    TABLE_SYMBOL,
    make_record_id,
};
use surrealdb::{Connection, Surreal};
use surrealdb::sql::{Id, Regex, Thing};
use uuid::Uuid;

/// Errors returned by the `SurrealDB` store implementation.
#[derive(Debug)]
pub enum StoreError {
    Surreal(Box<surrealdb::Error>),
    InvalidInput(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surreal(err) => write!(f, "SurrealDB error: {err}"),
            Self::InvalidInput(message) => write!(f, "Invalid input: {message}"),
        }
    }
}

impl Error for StoreError {}

impl From<surrealdb::Error> for StoreError {
    fn from(err: surrealdb::Error) -> Self {
        Self::Surreal(Box::new(err))
    }
}

pub type StoreResult<T> = Result<T, StoreError>;

/// Store implementation backed by `SurrealDB`.
pub struct SurrealDocStore<C: Connection> {
    db: Arc<Surreal<C>>,
}

impl<C: Connection> Clone for SurrealDocStore<C> {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
        }
    }
}

impl<C: Connection> SurrealDocStore<C> {
    #[must_use]
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            db: Arc::new(db),
        }
    }

    #[must_use]
    pub const fn from_arc(db: Arc<Surreal<C>>) -> Self {
        Self { db }
    }

    #[must_use]
    pub fn db(&self) -> &Surreal<C> {
        &self.db
    }

    /// Upserts a project record by id.
    ///
    /// # Errors
    /// Returns `StoreError` if validation fails or the database write fails.
    pub async fn upsert_project(&self, mut project: Project) -> StoreResult<Project> {
        ensure_non_empty(&project.project_id, "project_id")?;
        let id = project
            .id
            .clone()
            .unwrap_or_else(|| project.project_id.clone());
        project.id = Some(id.clone());
        let record = Thing::from((TABLE_PROJECT, id.as_str()));
        self.db
            .query("UPSERT $record CONTENT $data RETURN NONE;")
            .bind(("record", record))
            .bind(("data", project.clone()))
            .await?;
        Ok(project)
    }

    /// Fetches a project by id.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_project(&self, project_id: &str) -> StoreResult<Option<Project>> {
        let record: Option<Project> = self.db.select((TABLE_PROJECT, project_id)).await?;
        Ok(record)
    }

    /// Fetches an ingest record by id.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_ingest(&self, ingest_id: &str) -> StoreResult<Option<Ingest>> {
        let record = Thing::from((TABLE_INGEST, ingest_id));
        let mut response = self
            .db
            .query("SELECT * FROM $record;")
            .bind(("record", record))
            .await?;
        let records: Vec<IngestRow> = response.take(0)?;
        Ok(records.into_iter().next().map(Ingest::from))
    }

    /// Lists projects up to the provided limit.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit is invalid or the database query fails.
    pub async fn list_projects(&self, limit: usize) -> StoreResult<Vec<Project>> {
        let limit = limit_to_i64(limit)?;
        let query = "SELECT * FROM project LIMIT $limit;";
        let mut response = self.db.query(query).bind(("limit", limit)).await?;
        let records: Vec<Project> = response.take(0)?;
        Ok(records)
    }

    /// Searches projects by name or alias pattern.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit or pattern is invalid or the database query fails.
    pub async fn search_projects(&self, pattern: &str, limit: usize) -> StoreResult<Vec<Project>> {
        let Some(pattern) = normalize_pattern(pattern) else {
            return self.list_projects(limit).await;
        };
        let limit = limit_to_i64(limit)?;
        let regex = build_project_regex(&pattern)?;
        let query = "SELECT * FROM project WHERE search_text != NONE AND string::matches(search_text, $pattern) LIMIT $limit;";
        let mut response = self
            .db
            .query(query)
            .bind(("pattern", regex))
            .bind(("limit", limit))
            .await?;
        let records: Vec<Project> = response.take(0)?;
        Ok(records)
    }

    /// Lists ingest records for a project.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit is invalid or the database query fails.
    pub async fn list_ingests(&self, project_id: &str, limit: usize) -> StoreResult<Vec<Ingest>> {
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let query =
            "SELECT * FROM ingest WHERE project_id = $project_id ORDER BY ingested_at DESC LIMIT $limit;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("limit", limit))
            .await?;
        let records: Vec<IngestRow> = response.take(0)?;
        Ok(records.into_iter().map(Ingest::from).collect())
    }

    /// Creates an ingest record.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_ingest(&self, mut ingest: Ingest) -> StoreResult<Ingest> {
        let id = ingest.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        ingest.id = Some(id.clone());
        let record = Thing::from((TABLE_INGEST, id.as_str()));
        self.db
            .query("UPSERT $record CONTENT $data RETURN NONE;")
            .bind(("record", record))
            .bind(("data", ingest.clone()))
            .await?;
        Ok(ingest)
    }

    /// Creates a document source record.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_source(&self, mut source: DocSource) -> StoreResult<DocSource> {
        let id = source.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        source.id = Some(id.clone());
        self.db
            .query("CREATE doc_source CONTENT $data RETURN NONE;")
            .bind(("data", source.clone()))
            .await?;
        Ok(source)
    }

    /// Upserts a symbol record by symbol key.
    ///
    /// # Errors
    /// Returns `StoreError` if validation fails or the database write fails.
    pub async fn upsert_symbol(&self, mut symbol: Symbol) -> StoreResult<Symbol> {
        ensure_non_empty(&symbol.symbol_key, "symbol_key")?;
        let id = symbol
            .id
            .clone()
            .unwrap_or_else(|| symbol.symbol_key.clone());
        symbol.id = Some(id.clone());
        let record = Thing::from((TABLE_SYMBOL, id.as_str()));
        self.db
            .query("UPSERT $record CONTENT $data RETURN NONE;")
            .bind(("record", record))
            .bind(("data", symbol.clone()))
            .await?;
        Ok(symbol)
    }

    /// Creates a document block record.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_block(&self, mut block: DocBlock) -> StoreResult<DocBlock> {
        let id = block.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        block.id = Some(id.clone());
        self.db
            .query("CREATE doc_block CONTENT $data RETURN NONE;")
            .bind(("data", block.clone()))
            .await?;
        Ok(block)
    }

    /// Creates document block records.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_blocks(&self, blocks: Vec<DocBlock>) -> StoreResult<Vec<DocBlock>> {
        if blocks.is_empty() {
            return Ok(Vec::new());
        }
        let mut stored = Vec::with_capacity(blocks.len());
        for block in blocks {
            stored.push(self.create_doc_block(block).await?);
        }
        Ok(stored)
    }

    /// Creates document chunk records.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_chunks(&self, chunks: Vec<DocChunk>) -> StoreResult<Vec<DocChunk>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }
        let mut stored = Vec::with_capacity(chunks.len());
        for mut chunk in chunks {
            let id = chunk.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
            chunk.id = Some(id.clone());
            self.db
                .query("CREATE doc_chunk CONTENT $data RETURN NONE;")
                .bind(("data", chunk.clone()))
                .await?;
            stored.push(chunk);
        }
        Ok(stored)
    }

    /// Creates a relation record in the specified table.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_relation(
        &self,
        table: &str,
        mut relation: RelationRecord,
    ) -> StoreResult<RelationRecord> {
        let id = relation.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        relation.id = Some(id.clone());
        let statement = format!("CREATE {table} CONTENT $data RETURN NONE;");
        self.db
            .query(statement)
            .bind(("data", relation.clone()))
            .await?;
        Ok(relation)
    }

    /// Creates relation records in the specified table.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_relations(
        &self,
        table: &str,
        relations: Vec<RelationRecord>,
    ) -> StoreResult<Vec<RelationRecord>> {
        if relations.is_empty() {
            return Ok(Vec::new());
        }
        let mut stored = Vec::with_capacity(relations.len());
        for mut relation in relations {
            let id = relation.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
            relation.id = Some(id.clone());
            let statement = format!("CREATE {table} CONTENT $data RETURN NONE;");
            self.db
                .query(statement)
                .bind(("data", relation.clone()))
                .await?;
            stored.push(relation);
        }
        Ok(stored)
    }

    /// Fetches a symbol by key.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_symbol(&self, symbol_key: &str) -> StoreResult<Option<Symbol>> {
        let record: Option<Symbol> = self.db.select((TABLE_SYMBOL, symbol_key)).await?;
        Ok(record)
    }

    /// Fetches a symbol by project id and key.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_symbol_by_project(
        &self,
        project_id: &str,
        symbol_key: &str,
    ) -> StoreResult<Option<Symbol>> {
        let project_id = project_id.to_string();
        let symbol_key = symbol_key.to_string();
        let query = "SELECT * FROM symbol WHERE project_id = $project_id AND symbol_key = $symbol_key LIMIT 1;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("symbol_key", symbol_key))
            .await?;
        let mut records: Vec<Symbol> = response.take(0)?;
        Ok(records.pop())
    }

    /// Lists symbols by name match within a project.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit is invalid or the database query fails.
    pub async fn list_symbols_by_name(
        &self,
        project_id: &str,
        name: &str,
        limit: usize,
    ) -> StoreResult<Vec<Symbol>> {
        let project_id = project_id.to_string();
        let name = name.to_string();
        let limit = limit_to_i64(limit)?;
        let query = "SELECT * FROM symbol WHERE project_id = $project_id AND name CONTAINS $name LIMIT $limit;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("name", name))
            .bind(("limit", limit))
            .await?;
        let records: Vec<Symbol> = response.take(0)?;
        Ok(records)
    }

    /// Lists distinct symbol kinds for a project.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_symbol_kinds(&self, project_id: &str) -> StoreResult<Vec<String>> {
        let project_id = project_id.to_string();
        let query = "SELECT kind FROM symbol WHERE project_id = $project_id GROUP BY kind;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .await?;
        let records: Vec<SymbolKindRow> = response.take(0)?;
        let mut kinds: Vec<String> = records
            .into_iter()
            .filter_map(|row| row.kind)
            .filter(|value| !value.trim().is_empty())
            .collect();
        kinds.sort();
        kinds.dedup();
        Ok(kinds)
    }

    /// Lists members by scope prefix or glob pattern.
    ///
    /// # Errors
    /// Returns `StoreError` if the scope or limit is invalid or the database query fails.
    pub async fn list_members_by_scope(
        &self,
        project_id: &str,
        scope: &str,
        limit: usize,
    ) -> StoreResult<Vec<Symbol>> {
        let Some(scope) = normalize_pattern(scope) else {
            return Ok(Vec::new());
        };
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let mut response = if scope.contains('*') {
            let regex = build_scope_regex(&scope)?;
            let query = "SELECT * FROM symbol WHERE project_id = $project_id AND qualified_name != NONE AND string::matches(string::lowercase(qualified_name), $pattern) LIMIT $limit;";
            self.db
                .query(query)
                .bind(("project_id", project_id))
                .bind(("pattern", regex))
                .bind(("limit", limit))
                .await?
        } else {
            let query = "SELECT * FROM symbol WHERE project_id = $project_id AND qualified_name != NONE AND string::starts_with(string::lowercase(qualified_name), $scope) LIMIT $limit;";
            self.db
                .query(query)
                .bind(("project_id", project_id))
                .bind(("scope", scope))
                .bind(("limit", limit))
                .await?
        };
        let records: Vec<Symbol> = response.take(0)?;
        Ok(records)
    }

    /// Lists document blocks for a symbol, optionally filtering by ingest id.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_doc_blocks(
        &self,
        project_id: &str,
        symbol_key: &str,
        ingest_id: Option<&str>,
    ) -> StoreResult<Vec<DocBlock>> {
        let project_id = project_id.to_string();
        let symbol_key = symbol_key.to_string();
        let (query, binds) = ingest_id.map_or(
            (
                "SELECT * FROM doc_block WHERE project_id = $project_id AND symbol_key = $symbol_key;",
                None,
            ),
            |ingest_id| (
                "SELECT * FROM doc_block WHERE project_id = $project_id AND symbol_key = $symbol_key AND ingest_id = $ingest_id;",
                Some(ingest_id.to_string()),
            ),
        );
        let response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("symbol_key", symbol_key));
        let mut response = if let Some(ingest_id) = binds {
            response.bind(("ingest_id", ingest_id)).await?
        } else {
            response.await?
        };
        let records: Vec<DocBlock> = response.take(0)?;
        Ok(records)
    }

    /// Searches document blocks by text within a project.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit is invalid or the database query fails.
    pub async fn search_doc_blocks(
        &self,
        project_id: &str,
        text: &str,
        limit: usize,
    ) -> StoreResult<Vec<DocBlock>> {
        let project_id = project_id.to_string();
        let text = text.to_string();
        let limit = limit_to_i64(limit)?;
        let query = "SELECT * FROM doc_block WHERE project_id = $project_id AND (summary CONTAINS $text OR remarks CONTAINS $text OR returns CONTAINS $text) LIMIT $limit;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("text", text))
            .bind(("limit", limit))
            .await?;
        let records: Vec<DocBlock> = response.take(0)?;
        Ok(records)
    }

    /// Lists document sources by project and ingest ids.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_doc_sources(
        &self,
        project_id: &str,
        ingest_ids: &[String],
    ) -> StoreResult<Vec<DocSource>> {
        if ingest_ids.is_empty() {
            return Ok(Vec::new());
        }
        let project_id = project_id.to_string();
        let ingest_ids = ingest_ids.to_vec();
        let query = "SELECT * FROM doc_source WHERE project_id = $project_id AND ingest_id IN $ingest_ids;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("ingest_ids", ingest_ids))
            .await?;
        let records: Vec<DocSourceRow> = response.take(0)?;
        Ok(records.into_iter().map(DocSource::from).collect())
    }

    /// Fetches a document source by id.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_doc_source(&self, doc_source_id: &str) -> StoreResult<Option<DocSource>> {
        let record = Thing::from((TABLE_DOC_SOURCE, doc_source_id));
        let mut response = self
            .db
            .query("SELECT * FROM $record;")
            .bind(("record", record))
            .await?;
        let records: Vec<DocSourceRow> = response.take(0)?;
        Ok(records.into_iter().next().map(DocSource::from))
    }

    /// Lists document sources for a project, optionally filtered by ingest id.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit is invalid or the database query fails.
    pub async fn list_doc_sources_by_project(
        &self,
        project_id: &str,
        ingest_id: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<DocSource>> {
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let (query, binds) = ingest_id.map_or(
            (
                "SELECT * FROM doc_source WHERE project_id = $project_id ORDER BY source_modified_at DESC LIMIT $limit;",
                None,
            ),
            |ingest_id| (
                "SELECT * FROM doc_source WHERE project_id = $project_id AND ingest_id = $ingest_id ORDER BY source_modified_at DESC LIMIT $limit;",
                Some(ingest_id.to_string()),
            ),
        );
        let response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("limit", limit));
        let mut response = if let Some(ingest_id) = binds {
            response.bind(("ingest_id", ingest_id)).await?
        } else {
            response.await?
        };
        let records: Vec<DocSourceRow> = response.take(0)?;
        Ok(records.into_iter().map(DocSource::from).collect())
    }

    /// Lists relation records in a table where the symbol is the source (outgoing).
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_relations_from_symbol(
        &self,
        table: &str,
        project_id: &str,
        symbol_id: &str,
        limit: usize,
    ) -> StoreResult<Vec<RelationRecord>> {
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let record_id = make_record_id(TABLE_SYMBOL, symbol_id);
        let query = format!(
            "SELECT * FROM {table} WHERE project_id = $project_id AND out = $record_id LIMIT $limit;"
        );
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("record_id", record_id))
            .bind(("limit", limit))
            .await?;
        let records: Vec<RelationRecord> = response.take(0)?;
        Ok(records)
    }

    /// Lists relation records in a table where the symbol is the target (incoming).
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_relations_to_symbol(
        &self,
        table: &str,
        project_id: &str,
        symbol_id: &str,
        limit: usize,
    ) -> StoreResult<Vec<RelationRecord>> {
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let record_id = make_record_id(TABLE_SYMBOL, symbol_id);
        let query = format!(
            "SELECT * FROM {table} WHERE project_id = $project_id AND in = $record_id LIMIT $limit;"
        );
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("record_id", record_id))
            .bind(("limit", limit))
            .await?;
        let records: Vec<RelationRecord> = response.take(0)?;
        Ok(records)
    }

    /// Lists relation records for a document block id.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_relations_from_doc_block(
        &self,
        table: &str,
        project_id: &str,
        doc_block_id: &str,
        limit: usize,
    ) -> StoreResult<Vec<RelationRecord>> {
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let record_id = make_record_id(TABLE_DOC_BLOCK, doc_block_id);
        let query = format!(
            "SELECT * FROM {table} WHERE project_id = $project_id AND in = $record_id LIMIT $limit;"
        );
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("record_id", record_id))
            .bind(("limit", limit))
            .await?;
        let records: Vec<RelationRecord> = response.take(0)?;
        Ok(records)
    }
}

fn ensure_non_empty(value: &str, field: &str) -> StoreResult<()> {
    if value.is_empty() {
        return Err(StoreError::InvalidInput(format!("{field} is required")));
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct IngestRow {
    id: Thing,
    project_id: String,
    git_commit: Option<String>,
    git_branch: Option<String>,
    git_tag: Option<String>,
    project_version: Option<String>,
    source_modified_at: Option<String>,
    ingested_at: Option<String>,
    extra: Option<serde_json::Value>,
}

impl From<IngestRow> for Ingest {
    fn from(row: IngestRow) -> Self {
        Self {
            id: Some(thing_id_to_string(row.id)),
            project_id: row.project_id,
            git_commit: row.git_commit,
            git_branch: row.git_branch,
            git_tag: row.git_tag,
            project_version: row.project_version,
            source_modified_at: row.source_modified_at,
            ingested_at: row.ingested_at,
            extra: row.extra,
        }
    }
}

#[derive(serde::Deserialize)]
struct DocSourceRow {
    id: Thing,
    project_id: String,
    ingest_id: Option<String>,
    language: Option<String>,
    source_kind: Option<String>,
    path: Option<String>,
    tool_version: Option<String>,
    hash: Option<String>,
    source_modified_at: Option<String>,
    extra: Option<serde_json::Value>,
}

impl From<DocSourceRow> for DocSource {
    fn from(row: DocSourceRow) -> Self {
        Self {
            id: Some(thing_id_to_string(row.id)),
            project_id: row.project_id,
            ingest_id: row.ingest_id,
            language: row.language,
            source_kind: row.source_kind,
            path: row.path,
            tool_version: row.tool_version,
            hash: row.hash,
            source_modified_at: row.source_modified_at,
            extra: row.extra,
        }
    }
}

#[derive(serde::Deserialize)]
struct SymbolKindRow {
    kind: Option<String>,
}

fn normalize_pattern(pattern: &str) -> Option<String> {
    let trimmed = pattern.trim().to_lowercase();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn limit_to_i64(limit: usize) -> StoreResult<i64> {
    i64::try_from(limit).map_err(|_| {
        StoreError::InvalidInput("limit exceeds supported range".to_string())
    })
}

fn thing_id_to_string(thing: Thing) -> String {
    match thing.id {
        Id::String(value) => value,
        other => other.to_string(),
    }
}

fn build_project_regex(pattern: &str) -> StoreResult<Regex> {
    let body = glob_to_regex_body(pattern);
    let regex = format!(r"(^|\|){body}(\||$)");
    Regex::from_str(&regex).map_err(|err| {
        StoreError::InvalidInput(format!("Invalid project search pattern: {err}"))
    })
}

fn build_scope_regex(pattern: &str) -> StoreResult<Regex> {
    let body = glob_to_regex_body(pattern);
    let regex = format!(r"^{body}$");
    Regex::from_str(&regex).map_err(|err| {
        StoreError::InvalidInput(format!("Invalid scope search pattern: {err}"))
    })
}

fn glob_to_regex_body(pattern: &str) -> String {
    let mut escaped = String::new();
    for ch in pattern.chars() {
        match ch {
            '*' => escaped.push_str(".*"),
            '.' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use docx_store::models::{DocSource, Ingest};
    use surrealdb::engine::local::{Db, Mem};
    use surrealdb::Surreal;

    async fn build_store() -> SurrealDocStore<Db> {
        let db = Surreal::new::<Mem>(())
            .await
            .expect("failed to create in-memory SurrealDB");
        db.use_ns("docx")
            .use_db("test")
            .await
            .expect("failed to set namespace/db");
        SurrealDocStore::new(db)
    }

    #[tokio::test]
    async fn list_ingests_includes_ids() {
        let store = build_store().await;
        let ingest = Ingest {
            id: Some("ingest-1".to_string()),
            project_id: "project".to_string(),
            git_commit: None,
            git_branch: None,
            git_tag: None,
            project_version: None,
            source_modified_at: None,
            ingested_at: None,
            extra: None,
        };

        store
            .create_ingest(ingest)
            .await
            .expect("failed to create ingest");
        let ingests = store
            .list_ingests("project", 10)
            .await
            .expect("failed to list ingests");

        assert_eq!(ingests.len(), 1);
        assert_eq!(ingests[0].id.as_deref(), Some("ingest-1"));
    }

    #[tokio::test]
    async fn list_doc_sources_includes_ids() {
        let store = build_store().await;
        let source = DocSource {
            id: Some("source-1".to_string()),
            project_id: "project".to_string(),
            ingest_id: Some("ingest-1".to_string()),
            language: None,
            source_kind: None,
            path: None,
            tool_version: None,
            hash: None,
            source_modified_at: None,
            extra: None,
        };

        store
            .create_doc_source(source)
            .await
            .expect("failed to create doc source");
        let sources = store
            .list_doc_sources_by_project("project", None, 10)
            .await
            .expect("failed to list doc sources");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id.as_deref(), Some("source-1"));
    }
}
