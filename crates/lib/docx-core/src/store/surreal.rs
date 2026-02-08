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
    TABLE_DOC_CHUNK,
    TABLE_DOC_SOURCE,
    TABLE_INGEST,
    TABLE_PROJECT,
    TABLE_SYMBOL,
};
use surrealdb::{Connection, Surreal};
use surrealdb::sql::Regex;

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
    pub async fn upsert_project(&self, project: Project) -> StoreResult<Project> {
        ensure_non_empty(&project.project_id, "project_id")?;
        let fallback = project.clone();
        let record: Option<Project> = self
            .db
            .update((TABLE_PROJECT, project.project_id.clone()))
            .content(project)
            .await?;
        Ok(record.unwrap_or(fallback))
    }

    /// Fetches a project by id.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_project(&self, project_id: &str) -> StoreResult<Option<Project>> {
        let record: Option<Project> = self.db.select((TABLE_PROJECT, project_id)).await?;
        Ok(record)
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

    /// Creates an ingest record.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_ingest(&self, ingest: Ingest) -> StoreResult<Ingest> {
        let record: Option<Ingest> = self.db.create(TABLE_INGEST).content(ingest).await?;
        require_record(record, TABLE_INGEST)
    }

    /// Creates a document source record.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_source(&self, source: DocSource) -> StoreResult<DocSource> {
        let record: Option<DocSource> = self.db.create(TABLE_DOC_SOURCE).content(source).await?;
        require_record(record, TABLE_DOC_SOURCE)
    }

    /// Upserts a symbol record by symbol key.
    ///
    /// # Errors
    /// Returns `StoreError` if validation fails or the database write fails.
    pub async fn upsert_symbol(&self, symbol: Symbol) -> StoreResult<Symbol> {
        ensure_non_empty(&symbol.symbol_key, "symbol_key")?;
        let fallback = symbol.clone();
        let record: Option<Symbol> = self
            .db
            .update((TABLE_SYMBOL, symbol.symbol_key.clone()))
            .content(symbol)
            .await?;
        Ok(record.unwrap_or(fallback))
    }

    /// Creates a document block record.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_block(&self, block: DocBlock) -> StoreResult<DocBlock> {
        let record: Option<DocBlock> = self.db.create(TABLE_DOC_BLOCK).content(block).await?;
        require_record(record, TABLE_DOC_BLOCK)
    }

    /// Creates document block records.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_blocks(&self, blocks: Vec<DocBlock>) -> StoreResult<Vec<DocBlock>> {
        if blocks.is_empty() {
            return Ok(Vec::new());
        }
        let records: Option<Vec<DocBlock>> = self.db.create(TABLE_DOC_BLOCK).content(blocks).await?;
        require_record(records, TABLE_DOC_BLOCK)
    }

    /// Creates document chunk records.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_chunks(&self, chunks: Vec<DocChunk>) -> StoreResult<Vec<DocChunk>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }
        let records: Option<Vec<DocChunk>> = self.db.create(TABLE_DOC_CHUNK).content(chunks).await?;
        require_record(records, TABLE_DOC_CHUNK)
    }

    /// Creates a relation record in the specified table.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_relation(
        &self,
        table: &str,
        relation: RelationRecord,
    ) -> StoreResult<RelationRecord> {
        let record: Option<RelationRecord> = self.db.create(table).content(relation).await?;
        require_record(record, table)
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
        let records: Option<Vec<RelationRecord>> = self.db.create(table).content(relations).await?;
        require_record(records, table)
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
}

fn ensure_non_empty(value: &str, field: &str) -> StoreResult<()> {
    if value.is_empty() {
        return Err(StoreError::InvalidInput(format!("{field} is required")));
    }
    Ok(())
}

fn require_record<T>(record: Option<T>, table: &str) -> StoreResult<T> {
    record.ok_or_else(|| {
        StoreError::InvalidInput(format!(
            "No record returned when creating {table}"
        ))
    })
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
