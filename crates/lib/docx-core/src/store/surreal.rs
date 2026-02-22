use std::{collections::HashSet, error::Error, fmt, str::FromStr, sync::Arc};

use docx_store::models::{DocBlock, DocChunk, DocSource, Ingest, Project, RelationRecord, Symbol};
use docx_store::schema::{
    SCHEMA_BOOTSTRAP_SURQL, TABLE_DOC_BLOCK, TABLE_DOC_SOURCE, TABLE_INGEST, TABLE_PROJECT,
    TABLE_SYMBOL,
};
use serde::Serialize;
use serde_json::Value;
use surrealdb::types::{RecordId, RecordIdKey, Regex, SurrealValue, Table, ToSql};
use surrealdb::{Connection, Surreal};
use tracing::warn;
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

const OPTIONAL_DOC_BLOCK_FTS_START: &str = "-- OPTIONAL_DOC_BLOCK_FTS_START";
const OPTIONAL_DOC_BLOCK_FTS_END: &str = "-- OPTIONAL_DOC_BLOCK_FTS_END";

/// Store implementation backed by `SurrealDB`.
pub struct SurrealDocStore<C: Connection> {
    db: Arc<Surreal<C>>,
    schema_ready: Arc<tokio::sync::OnceCell<()>>,
}

impl<C: Connection> Clone for SurrealDocStore<C> {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            schema_ready: self.schema_ready.clone(),
        }
    }
}

impl<C: Connection> SurrealDocStore<C> {
    #[must_use]
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            db: Arc::new(db),
            schema_ready: Arc::new(tokio::sync::OnceCell::new()),
        }
    }

    #[must_use]
    pub fn from_arc(db: Arc<Surreal<C>>) -> Self {
        Self {
            db,
            schema_ready: Arc::new(tokio::sync::OnceCell::new()),
        }
    }

    #[must_use]
    pub fn db(&self) -> &Surreal<C> {
        &self.db
    }

    async fn ensure_schema(&self) -> StoreResult<()> {
        self.schema_ready
            .get_or_try_init(|| async {
                let (required_schema, optional_doc_block_fts) =
                    split_optional_doc_block_fts_schema(SCHEMA_BOOTSTRAP_SURQL)?;
                apply_schema(self.db.as_ref(), required_schema.as_str()).await?;
                if let Some(optional_doc_block_fts) = optional_doc_block_fts
                    && let Err(error) =
                        apply_schema(self.db.as_ref(), optional_doc_block_fts.as_str()).await
                {
                    warn!(
                        error = %error,
                        "optional doc_block full-text schema was skipped"
                    );
                }
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(())
    }

    /// Upserts a project record by id.
    ///
    /// # Errors
    /// Returns `StoreError` if validation fails or the database write fails.
    pub async fn upsert_project(&self, mut project: Project) -> StoreResult<Project> {
        self.ensure_schema().await?;
        ensure_non_empty(&project.project_id, "project_id")?;
        let id = project
            .id
            .clone()
            .unwrap_or_else(|| project.project_id.clone());
        project.id = Some(id.clone());
        let record = RecordId::new(TABLE_PROJECT, id.as_str());
        self.db
            .query("UPSERT $record CONTENT $data RETURN NONE;")
            .bind(("record", record))
            .bind(("data", project.clone()))
            .await?
            .check()?;
        Ok(project)
    }

    /// Fetches a project by id.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_project(&self, project_id: &str) -> StoreResult<Option<Project>> {
        self.ensure_schema().await?;
        let record = RecordId::new(TABLE_PROJECT, project_id);
        let mut response = self
            .db
            .query("SELECT *, record::id(id) AS id FROM $record;")
            .bind(("record", record))
            .await?;
        let mut records: Vec<Project> = response.take(0)?;
        Ok(records.pop())
    }

    /// Fetches an ingest record by id.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_ingest(&self, ingest_id: &str) -> StoreResult<Option<Ingest>> {
        self.ensure_schema().await?;
        let record = RecordId::new(TABLE_INGEST, ingest_id);
        let mut response = self
            .db
            .query("SELECT * FROM $record;")
            .bind(("record", record))
            .await?;
        let records: Vec<IngestRow> = response.take(0)?;
        if let Some(ingest) = records.into_iter().next().map(Ingest::from) {
            return Ok(Some(ingest));
        }
        if ingest_id.contains("::") {
            return Ok(None);
        }

        let mut response = self
            .db
            .query("SELECT * FROM ingest WHERE extra.requested_ingest_id = $requested_id;")
            .bind(("requested_id", ingest_id.to_string()))
            .await?;
        let records: Vec<IngestRow> = response.take(0)?;
        let mut rows = records.into_iter();
        let first = rows.next();
        if rows.next().is_some() {
            return Err(StoreError::InvalidInput(format!(
                "ingest_id '{ingest_id}' is ambiguous; use project-scoped ingest id"
            )));
        }
        Ok(first.map(Ingest::from))
    }

    /// Lists projects up to the provided limit.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit is invalid or the database query fails.
    pub async fn list_projects(&self, limit: usize) -> StoreResult<Vec<Project>> {
        self.ensure_schema().await?;
        let limit = limit_to_i64(limit)?;
        let query = "SELECT *, record::id(id) AS id FROM project LIMIT $limit;";
        let mut response = self.db.query(query).bind(("limit", limit)).await?;
        let records: Vec<Project> = response.take(0)?;
        Ok(records)
    }

    /// Searches projects by name or alias pattern.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit or pattern is invalid or the database query fails.
    pub async fn search_projects(&self, pattern: &str, limit: usize) -> StoreResult<Vec<Project>> {
        self.ensure_schema().await?;
        let Some(pattern) = normalize_pattern(pattern) else {
            return self.list_projects(limit).await;
        };
        let limit = limit_to_i64(limit)?;
        let regex = build_project_regex(&pattern)?;
        let query = format!(
            "SELECT *, record::id(id) AS id FROM project WHERE search_text != NONE AND string::matches(search_text, {}) LIMIT $limit;",
            regex.to_sql()
        );
        let mut response = self.db.query(query).bind(("limit", limit)).await?;
        let records: Vec<Project> = response.take(0)?;
        Ok(records)
    }

    /// Lists ingest records for a project.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit is invalid or the database query fails.
    pub async fn list_ingests(&self, project_id: &str, limit: usize) -> StoreResult<Vec<Ingest>> {
        self.ensure_schema().await?;
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let query = "SELECT * FROM ingest WHERE project_id = $project_id ORDER BY ingested_at DESC LIMIT $limit;";
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
        self.ensure_schema().await?;
        let provided_id = ingest.id.clone();
        let id = provided_id.as_ref().map_or_else(
            || Uuid::new_v4().to_string(),
            |value| make_scoped_ingest_id(&ingest.project_id, value),
        );
        if let Some(provided_id) = provided_id
            && provided_id != id
        {
            ingest.extra = Some(merge_ingest_extra(ingest.extra.take(), &provided_id));
        }
        ingest.id = Some(id.clone());
        let record = RecordId::new(TABLE_INGEST, id.as_str());
        self.db
            .query("UPSERT $record CONTENT $data RETURN NONE;")
            .bind(("record", record))
            .bind(("data", ingest.clone()))
            .await?
            .check()?;
        Ok(ingest)
    }

    /// Creates a document source record.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_source(&self, mut source: DocSource) -> StoreResult<DocSource> {
        self.ensure_schema().await?;
        let id = source
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        source.id = Some(id.clone());
        self.db
            .query("CREATE doc_source CONTENT $data RETURN NONE;")
            .bind(("data", source.clone()))
            .await?
            .check()?;
        Ok(source)
    }

    /// Upserts a symbol record by symbol key.
    ///
    /// # Errors
    /// Returns `StoreError` if validation fails or the database write fails.
    pub async fn upsert_symbol(&self, mut symbol: Symbol) -> StoreResult<Symbol> {
        self.ensure_schema().await?;
        ensure_non_empty(&symbol.symbol_key, "symbol_key")?;
        let id = symbol
            .id
            .clone()
            .unwrap_or_else(|| symbol.symbol_key.clone());
        symbol.id = Some(id.clone());
        let record = RecordId::new(TABLE_SYMBOL, id.as_str());
        self.db
            .query("UPSERT $record CONTENT $data RETURN NONE;")
            .bind(("record", record))
            .bind(("data", symbol.clone()))
            .await?
            .check()?;
        Ok(symbol)
    }

    /// Creates a document block record.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_block(&self, mut block: DocBlock) -> StoreResult<DocBlock> {
        self.ensure_schema().await?;
        let id = block
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        block.id = Some(id.clone());
        self.db
            .query("CREATE doc_block CONTENT $data RETURN NONE;")
            .bind(("data", block.clone()))
            .await?
            .check()?;
        Ok(block)
    }

    /// Creates document block records concurrently.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_blocks(&self, blocks: Vec<DocBlock>) -> StoreResult<Vec<DocBlock>> {
        self.ensure_schema().await?;
        if blocks.is_empty() {
            return Ok(Vec::new());
        }
        let futs: Vec<_> = blocks
            .into_iter()
            .map(|block| self.create_doc_block(block))
            .collect();
        let results = futures::future::join_all(futs).await;
        results.into_iter().collect()
    }

    /// Creates document chunk records.
    ///
    /// # Errors
    /// Returns `StoreError` if the database write fails.
    pub async fn create_doc_chunks(&self, chunks: Vec<DocChunk>) -> StoreResult<Vec<DocChunk>> {
        self.ensure_schema().await?;
        if chunks.is_empty() {
            return Ok(Vec::new());
        }
        let mut stored = Vec::with_capacity(chunks.len());
        for mut chunk in chunks {
            let id = chunk
                .id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            chunk.id = Some(id.clone());
            self.db
                .query("CREATE doc_chunk CONTENT $data RETURN NONE;")
                .bind(("data", chunk.clone()))
                .await?
                .check()?;
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
        relation: RelationRecord,
    ) -> StoreResult<RelationRecord> {
        self.ensure_schema().await?;
        ensure_identifier(table, "table")?;
        let in_id = parse_record_id(&relation.in_id, "in_id")?;
        let out_id = parse_record_id(&relation.out_id, "out_id")?;
        let payload = RelationPayload::from(&relation);
        let statement = format!("RELATE $in->{table}->$out CONTENT $data RETURN NONE;");
        self.db
            .query(statement)
            .bind(("in", in_id))
            .bind(("out", out_id))
            .bind(("data", payload))
            .await?
            .check()?;
        Ok(relation)
    }

    /// Creates relation records in the specified table concurrently.
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
        let futs: Vec<_> = relations
            .into_iter()
            .map(|r| self.create_relation(table, r))
            .collect();
        let results = futures::future::join_all(futs).await;
        results.into_iter().collect()
    }

    /// Removes a database in the current namespace.
    ///
    /// # Errors
    /// Returns `StoreError` if the input is invalid or the query fails.
    pub async fn remove_database(&self, db_name: &str) -> StoreResult<()> {
        ensure_non_empty(db_name, "db_name")?;
        let identifier = Table::from(db_name).to_sql();
        let statement = format!("REMOVE DATABASE {identifier};");
        self.db.query(statement).await?.check()?;
        Ok(())
    }

    /// Fetches a symbol by key.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_symbol(&self, symbol_key: &str) -> StoreResult<Option<Symbol>> {
        self.ensure_schema().await?;
        let record = RecordId::new(TABLE_SYMBOL, symbol_key);
        let mut response = self
            .db
            .query("SELECT *, record::id(id) AS id FROM $record;")
            .bind(("record", record))
            .await?;
        let mut records: Vec<Symbol> = response.take(0)?;
        Ok(records.pop())
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
        self.ensure_schema().await?;
        let project_id = project_id.to_string();
        let symbol_key = symbol_key.to_string();
        let query = "SELECT *, record::id(id) AS id FROM symbol WHERE project_id = $project_id AND symbol_key = $symbol_key LIMIT 1;";
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
        self.ensure_schema().await?;
        let project_id = project_id.to_string();
        let name = name.to_string();
        let limit = limit_to_i64(limit)?;
        let query = "SELECT *, record::id(id) AS id FROM symbol WHERE project_id = $project_id AND name CONTAINS $name LIMIT $limit;";
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

    /// Searches symbols with multiple optional filters.
    ///
    /// # Errors
    /// Returns `StoreError` if the limit is invalid or the database query fails.
    pub async fn search_symbols_advanced(
        &self,
        project_id: &str,
        name: Option<&str>,
        qualified_name: Option<&str>,
        symbol_key: Option<&str>,
        signature: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<Symbol>> {
        self.ensure_schema().await?;
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;

        let mut clauses = vec!["project_id = $project_id".to_string()];
        if symbol_key.is_some() {
            clauses.push("symbol_key = $symbol_key".to_string());
        }
        if name.is_some() {
            clauses.push(
                "name != NONE AND string::contains(string::lowercase(name), string::lowercase($name))"
                    .to_string(),
            );
        }
        if qualified_name.is_some() {
            clauses.push(
                "qualified_name != NONE AND string::contains(string::lowercase(qualified_name), string::lowercase($qualified_name))"
                    .to_string(),
            );
        }
        if signature.is_some() {
            clauses.push(
                "signature != NONE AND string::contains(string::lowercase(signature), string::lowercase($signature))"
                    .to_string(),
            );
        }

        let query = format!(
            "SELECT *, record::id(id) AS id FROM symbol WHERE {} LIMIT $limit;",
            clauses.join(" AND ")
        );

        let mut request = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("limit", limit));
        if let Some(value) = symbol_key {
            request = request.bind(("symbol_key", value.to_string()));
        }
        if let Some(value) = name {
            request = request.bind(("name", value.to_string()));
        }
        if let Some(value) = qualified_name {
            request = request.bind(("qualified_name", value.to_string()));
        }
        if let Some(value) = signature {
            request = request.bind(("signature", value.to_string()));
        }

        let mut response = request.await?;
        let records: Vec<Symbol> = response.take(0)?;
        Ok(records)
    }

    /// Lists distinct symbol kinds for a project.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_symbol_kinds(&self, project_id: &str) -> StoreResult<Vec<String>> {
        self.ensure_schema().await?;
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
        self.ensure_schema().await?;
        let Some(scope) = normalize_pattern(scope) else {
            return Ok(Vec::new());
        };
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let mut response = if scope.contains('*') {
            let regex = build_scope_regex(&scope)?;
            let query = format!(
                "SELECT *, record::id(id) AS id FROM symbol WHERE project_id = $project_id AND qualified_name != NONE AND string::matches(string::lowercase(qualified_name), {}) LIMIT $limit;",
                regex.to_sql()
            );
            self.db
                .query(query)
                .bind(("project_id", project_id))
                .bind(("limit", limit))
                .await?
        } else {
            let query = "SELECT *, record::id(id) AS id FROM symbol WHERE project_id = $project_id AND qualified_name != NONE AND string::starts_with(string::lowercase(qualified_name), $scope) LIMIT $limit;";
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
        self.ensure_schema().await?;
        let project_id = project_id.to_string();
        let symbol_key = symbol_key.to_string();
        let (query, binds) = ingest_id.map_or(
            (
                "SELECT *, record::id(id) AS id FROM doc_block WHERE project_id = $project_id AND symbol_key = $symbol_key;",
                None,
            ),
            |ingest_id| (
                "SELECT *, record::id(id) AS id FROM doc_block WHERE project_id = $project_id AND symbol_key = $symbol_key AND ingest_id = $ingest_id;",
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
        self.ensure_schema().await?;
        let project_id = project_id.to_string();
        let text = text.to_string();
        let limit = limit_to_i64(limit)?;
        let query = "\
            SELECT *, record::id(id) AS id FROM doc_block \
            WHERE project_id = $project_id \
              AND (string::contains(string::lowercase(summary ?? ''), string::lowercase($text)) \
                OR string::contains(string::lowercase(remarks ?? ''), string::lowercase($text)) \
                OR string::contains(string::lowercase(returns ?? ''), string::lowercase($text)) \
                OR string::contains(string::lowercase(errors ?? ''), string::lowercase($text)) \
                OR string::contains(string::lowercase(panics ?? ''), string::lowercase($text)) \
                OR string::contains(string::lowercase(safety ?? ''), string::lowercase($text))) \
            LIMIT $limit;";
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
        self.ensure_schema().await?;
        if ingest_ids.is_empty() {
            return Ok(Vec::new());
        }
        let project_id = project_id.to_string();
        let ingest_ids = normalize_ingest_filter_ids(project_id.as_str(), ingest_ids);
        if ingest_ids.is_empty() {
            return Ok(Vec::new());
        }
        let query =
            "SELECT * FROM doc_source WHERE project_id = $project_id AND ingest_id IN $ingest_ids;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("ingest_ids", ingest_ids))
            .await?;
        let records: Vec<DocSourceRow> = response.take(0)?;
        Ok(records.into_iter().map(DocSource::from).collect())
    }

    /// Lists document sources by explicit source ids.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_doc_sources_by_ids(
        &self,
        project_id: &str,
        doc_source_ids: &[String],
    ) -> StoreResult<Vec<DocSource>> {
        self.ensure_schema().await?;
        if doc_source_ids.is_empty() {
            return Ok(Vec::new());
        }
        let project_id = project_id.to_string();
        let mut unique_ids = HashSet::new();
        let records: Vec<RecordId> = doc_source_ids
            .iter()
            .filter(|value| !value.is_empty())
            .filter(|value| unique_ids.insert((*value).clone()))
            .map(|value| RecordId::new(TABLE_DOC_SOURCE, value.as_str()))
            .collect();
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let query = "SELECT * FROM doc_source WHERE project_id = $project_id AND id IN $records;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("records", records))
            .await?;
        let records: Vec<DocSourceRow> = response.take(0)?;
        Ok(records.into_iter().map(DocSource::from).collect())
    }

    /// Counts table rows for a project.
    ///
    /// # Errors
    /// Returns `StoreError` if the input is invalid or the database query fails.
    pub async fn count_rows_for_project(
        &self,
        table: &str,
        project_id: &str,
    ) -> StoreResult<usize> {
        self.ensure_schema().await?;
        ensure_identifier(table, "table")?;
        let query = format!(
            "SELECT count() AS count FROM {table} WHERE project_id = $project_id GROUP ALL;"
        );
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .await?;
        let rows: Vec<CountRow> = response.take(0)?;
        Ok(rows
            .first()
            .and_then(|row| usize::try_from(row.count).ok())
            .unwrap_or(0))
    }

    /// Counts symbols in a project where a given field is missing (`NONE`).
    ///
    /// # Errors
    /// Returns `StoreError` if the input is invalid or the database query fails.
    pub async fn count_symbols_missing_field(
        &self,
        project_id: &str,
        field: &str,
    ) -> StoreResult<usize> {
        self.ensure_schema().await?;
        ensure_identifier(field, "field")?;
        let query = format!(
            "SELECT count() AS count FROM symbol WHERE project_id = $project_id AND {field} = NONE GROUP ALL;"
        );
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .await?;
        let rows: Vec<CountRow> = response.take(0)?;
        Ok(rows
            .first()
            .and_then(|row| usize::try_from(row.count).ok())
            .unwrap_or(0))
    }

    /// Lists non-null symbol keys attached to doc blocks for a project.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_doc_block_symbol_keys(&self, project_id: &str) -> StoreResult<Vec<String>> {
        self.ensure_schema().await?;
        let mut response = self
            .db
            .query(
                "SELECT symbol_key FROM doc_block WHERE project_id = $project_id AND symbol_key != NONE;",
            )
            .bind(("project_id", project_id.to_string()))
            .await?;
        let rows: Vec<DocBlockSymbolKeyRow> = response.take(0)?;
        Ok(rows.into_iter().map(|row| row.symbol_key).collect())
    }

    /// Lists observed-in source relation endpoint ids (`symbol:<id>`) for a project.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn list_observed_in_symbol_refs(&self, project_id: &str) -> StoreResult<Vec<String>> {
        self.ensure_schema().await?;
        let mut response = self
            .db
            .query("SELECT in AS symbol_id FROM observed_in WHERE project_id = $project_id;")
            .bind(("project_id", project_id.to_string()))
            .await?;
        let rows: Vec<ObservedInSymbolRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| record_id_to_record_ref(row.symbol_id))
            .collect())
    }

    /// Fetches a document source by id.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    pub async fn get_doc_source(&self, doc_source_id: &str) -> StoreResult<Option<DocSource>> {
        self.ensure_schema().await?;
        let record = RecordId::new(TABLE_DOC_SOURCE, doc_source_id);
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
        self.ensure_schema().await?;
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let (query, ingest_ids) = ingest_id.map_or(
            (
                "SELECT * FROM doc_source WHERE project_id = $project_id ORDER BY source_modified_at DESC LIMIT $limit;",
                None,
            ),
            |ingest_id| {
                (
                    "SELECT * FROM doc_source WHERE project_id = $project_id AND ingest_id IN $ingest_ids ORDER BY source_modified_at DESC LIMIT $limit;",
                    Some(normalize_ingest_filter_ids(
                        project_id.as_str(),
                        &[ingest_id.to_string()],
                    )),
                )
            },
        );
        if ingest_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Vec::new());
        }
        let response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("limit", limit));
        let mut response = if let Some(ingest_ids) = ingest_ids {
            response.bind(("ingest_ids", ingest_ids)).await?
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
        self.ensure_schema().await?;
        ensure_identifier(table, "table")?;
        let limit = limit_to_i64(limit)?;
        let record_id = RecordId::new(TABLE_SYMBOL, symbol_id);
        let query = format!(
            "SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $record->{table} WHERE project_id = $project_id LIMIT $limit;"
        );
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .bind(("record", record_id))
            .bind(("limit", limit))
            .await?;
        let records: Vec<RelationRow> = response.take(0)?;
        Ok(records.into_iter().map(RelationRecord::from).collect())
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
        self.ensure_schema().await?;
        ensure_identifier(table, "table")?;
        let limit = limit_to_i64(limit)?;
        let record_id = RecordId::new(TABLE_SYMBOL, symbol_id);
        let query = format!(
            "SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $record<-{table} WHERE project_id = $project_id LIMIT $limit;"
        );
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .bind(("record", record_id))
            .bind(("limit", limit))
            .await?;
        let records: Vec<RelationRow> = response.take(0)?;
        Ok(records.into_iter().map(RelationRecord::from).collect())
    }

    /// Fetches all adjacency relations for a symbol in a single multi-statement query.
    ///
    /// # Errors
    /// Returns `StoreError` if the database query fails.
    #[allow(clippy::too_many_lines)]
    pub async fn fetch_symbol_adjacency(
        &self,
        symbol_id: &str,
        project_id: &str,
        limit: usize,
    ) -> StoreResult<AdjacencyRaw> {
        self.ensure_schema().await?;
        let limit = limit_to_i64(limit)?;
        let record = RecordId::new(TABLE_SYMBOL, symbol_id);
        let query = r"
            LET $sym = $record;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym->member_of   WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym<-member_of   WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym->contains    WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym<-contains    WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym->returns     WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym<-returns     WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym->param_type  WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym<-param_type  WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym->see_also    WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym<-see_also    WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym->inherits    WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym<-inherits    WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym->references  WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym<-references  WHERE project_id = $project_id LIMIT $limit;
            SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM $sym->observed_in WHERE project_id = $project_id LIMIT $limit;
        ";
        let mut response = self
            .db
            .query(query)
            .bind(("record", record))
            .bind(("project_id", project_id.to_string()))
            .bind(("limit", limit))
            .await?;

        // Statement 0 is LET, statements 1..=15 are SELECTs
        let member_of_out: Vec<RelationRow> = response.take(1)?;
        let member_of_in: Vec<RelationRow> = response.take(2)?;
        let contains_out: Vec<RelationRow> = response.take(3)?;
        let contains_in: Vec<RelationRow> = response.take(4)?;
        let returns_out: Vec<RelationRow> = response.take(5)?;
        let returns_in: Vec<RelationRow> = response.take(6)?;
        let param_types_out: Vec<RelationRow> = response.take(7)?;
        let param_types_in: Vec<RelationRow> = response.take(8)?;
        let see_also_out: Vec<RelationRow> = response.take(9)?;
        let see_also_in: Vec<RelationRow> = response.take(10)?;
        let inherits_out: Vec<RelationRow> = response.take(11)?;
        let inherits_in: Vec<RelationRow> = response.take(12)?;
        let references_out: Vec<RelationRow> = response.take(13)?;
        let references_in: Vec<RelationRow> = response.take(14)?;
        let observed_in_out: Vec<RelationRow> = response.take(15)?;

        let to_records = |rows: Vec<RelationRow>| -> Vec<RelationRecord> {
            rows.into_iter().map(RelationRecord::from).collect()
        };

        Ok(AdjacencyRaw {
            member_of: merge_relation_rows(to_records(member_of_out), to_records(member_of_in)),
            contains: merge_relation_rows(to_records(contains_out), to_records(contains_in)),
            returns: merge_relation_rows(to_records(returns_out), to_records(returns_in)),
            param_types: merge_relation_rows(
                to_records(param_types_out),
                to_records(param_types_in),
            ),
            see_also: merge_relation_rows(to_records(see_also_out), to_records(see_also_in)),
            inherits: merge_relation_rows(to_records(inherits_out), to_records(inherits_in)),
            references: merge_relation_rows(to_records(references_out), to_records(references_in)),
            observed_in: to_records(observed_in_out),
        })
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
        self.ensure_schema().await?;
        ensure_identifier(table, "table")?;
        let project_id = project_id.to_string();
        let limit = limit_to_i64(limit)?;
        let record_id = RecordId::new(TABLE_DOC_BLOCK, doc_block_id);
        let query = format!(
            "SELECT id, in AS in_id, out AS out_id, project_id, ingest_id, kind, extra FROM {table} WHERE project_id = $project_id AND in = $record_id LIMIT $limit;"
        );
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("record_id", record_id))
            .bind(("limit", limit))
            .await?;
        let records: Vec<RelationRow> = response.take(0)?;
        Ok(records.into_iter().map(RelationRecord::from).collect())
    }
}

/// Raw adjacency data returned from a single multi-statement query.
#[derive(Debug, Default)]
pub struct AdjacencyRaw {
    pub member_of: Vec<RelationRecord>,
    pub contains: Vec<RelationRecord>,
    pub returns: Vec<RelationRecord>,
    pub param_types: Vec<RelationRecord>,
    pub see_also: Vec<RelationRecord>,
    pub inherits: Vec<RelationRecord>,
    pub references: Vec<RelationRecord>,
    pub observed_in: Vec<RelationRecord>,
}

fn merge_relation_rows(
    mut left: Vec<RelationRecord>,
    right: Vec<RelationRecord>,
) -> Vec<RelationRecord> {
    let mut seen = std::collections::HashSet::new();
    for r in &left {
        seen.insert((r.in_id.clone(), r.out_id.clone(), r.kind.clone()));
    }
    for r in right {
        let key = (r.in_id.clone(), r.out_id.clone(), r.kind.clone());
        if seen.insert(key) {
            left.push(r);
        }
    }
    left
}

fn ensure_non_empty(value: &str, field: &str) -> StoreResult<()> {
    if value.is_empty() {
        return Err(StoreError::InvalidInput(format!("{field} is required")));
    }
    Ok(())
}

fn ensure_identifier(value: &str, field: &str) -> StoreResult<()> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(StoreError::InvalidInput(format!(
            "{field} must be a valid identifier"
        )));
    }
    Ok(())
}

fn parse_record_id(value: &str, field: &str) -> StoreResult<RecordId> {
    ensure_non_empty(value, field)?;
    RecordId::parse_simple(value).map_err(|err| {
        StoreError::InvalidInput(format!(
            "{field} must be a record id in 'table:key' format: {err}"
        ))
    })
}

#[derive(Debug, Clone, Serialize, SurrealValue)]
struct RelationPayload {
    project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ingest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<Value>,
}

impl From<&RelationRecord> for RelationPayload {
    fn from(value: &RelationRecord) -> Self {
        Self {
            project_id: value.project_id.clone(),
            ingest_id: value.ingest_id.clone(),
            kind: value.kind.clone(),
            extra: value.extra.clone(),
        }
    }
}

#[derive(serde::Deserialize, SurrealValue)]
struct RelationRow {
    id: RecordId,
    in_id: RecordId,
    out_id: RecordId,
    project_id: String,
    ingest_id: Option<String>,
    kind: Option<String>,
    extra: Option<Value>,
}

impl From<RelationRow> for RelationRecord {
    fn from(row: RelationRow) -> Self {
        Self {
            id: Some(record_id_to_string(row.id)),
            in_id: record_id_to_record_ref(row.in_id),
            out_id: record_id_to_record_ref(row.out_id),
            project_id: row.project_id,
            ingest_id: row.ingest_id,
            kind: row.kind,
            extra: row.extra,
        }
    }
}

#[derive(serde::Deserialize, SurrealValue)]
struct IngestRow {
    id: RecordId,
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
            id: Some(record_id_to_string(row.id)),
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

#[derive(serde::Deserialize, SurrealValue)]
struct DocSourceRow {
    id: RecordId,
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
            id: Some(record_id_to_string(row.id)),
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

#[derive(serde::Deserialize, SurrealValue)]
struct SymbolKindRow {
    kind: Option<String>,
}

#[derive(serde::Deserialize, SurrealValue)]
struct CountRow {
    count: i64,
}

#[derive(serde::Deserialize, SurrealValue)]
struct DocBlockSymbolKeyRow {
    symbol_key: String,
}

#[derive(serde::Deserialize, SurrealValue)]
struct ObservedInSymbolRow {
    symbol_id: RecordId,
}

async fn apply_schema<C: Connection>(db: &Surreal<C>, schema: &str) -> StoreResult<()> {
    db.query(schema).await?.check()?;
    Ok(())
}

fn split_optional_doc_block_fts_schema(schema: &str) -> StoreResult<(String, Option<String>)> {
    let mut required_schema = String::new();
    let mut optional_schema = String::new();
    let mut in_optional_block = false;
    let mut found_optional_start = false;
    let mut found_optional_end = false;

    for line in schema.lines() {
        let trimmed = line.trim();
        if trimmed == OPTIONAL_DOC_BLOCK_FTS_START {
            if in_optional_block || found_optional_start {
                return Err(StoreError::InvalidInput(
                    "schema optional FTS block start marker appears multiple times".to_string(),
                ));
            }
            found_optional_start = true;
            in_optional_block = true;
            continue;
        }
        if trimmed == OPTIONAL_DOC_BLOCK_FTS_END {
            if !in_optional_block {
                return Err(StoreError::InvalidInput(
                    "schema optional FTS block end marker appears before start".to_string(),
                ));
            }
            found_optional_end = true;
            in_optional_block = false;
            continue;
        }
        if in_optional_block {
            optional_schema.push_str(line);
            optional_schema.push('\n');
        } else {
            required_schema.push_str(line);
            required_schema.push('\n');
        }
    }

    if in_optional_block {
        return Err(StoreError::InvalidInput(
            "schema optional FTS block start marker is missing a matching end marker".to_string(),
        ));
    }
    if found_optional_start != found_optional_end {
        return Err(StoreError::InvalidInput(
            "schema optional FTS block markers are unbalanced".to_string(),
        ));
    }
    if !found_optional_start {
        return Ok((required_schema, None));
    }

    let optional_schema = optional_schema.trim();
    if optional_schema.is_empty() {
        return Err(StoreError::InvalidInput(
            "schema optional FTS block is empty".to_string(),
        ));
    }
    Ok((required_schema, Some(optional_schema.to_string())))
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
    i64::try_from(limit)
        .map_err(|_| StoreError::InvalidInput("limit exceeds supported range".to_string()))
}

fn record_id_to_string(record_id: RecordId) -> String {
    record_id_key_to_string(record_id.key)
}

fn record_id_to_record_ref(record_id: RecordId) -> String {
    let table = record_id.table.into_string();
    let key = record_id_key_to_string(record_id.key);
    format!("{table}:{key}")
}

fn record_id_key_to_string(key: RecordIdKey) -> String {
    match key {
        RecordIdKey::String(value) => value,
        other => other.to_sql(),
    }
}

fn build_project_regex(pattern: &str) -> StoreResult<Regex> {
    let body = glob_to_regex_body(pattern);
    let regex = format!(r"(^|\|){body}(\||$)");
    Regex::from_str(&regex)
        .map_err(|err| StoreError::InvalidInput(format!("Invalid project search pattern: {err}")))
}

fn build_scope_regex(pattern: &str) -> StoreResult<Regex> {
    let body = glob_to_regex_body(pattern);
    let regex = format!(r"^{body}$");
    Regex::from_str(&regex)
        .map_err(|err| StoreError::InvalidInput(format!("Invalid scope search pattern: {err}")))
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

fn make_scoped_ingest_id(project_id: &str, ingest_id: &str) -> String {
    let prefix = format!("{project_id}::");
    if ingest_id.starts_with(prefix.as_str()) {
        ingest_id.to_string()
    } else {
        format!("{project_id}::{ingest_id}")
    }
}

fn normalize_ingest_filter_ids(project_id: &str, ingest_ids: &[String]) -> Vec<String> {
    let scoped_prefix = format!("{project_id}::");
    let mut unique = HashSet::new();
    let mut normalized = Vec::new();
    for ingest_id in ingest_ids {
        let trimmed = ingest_id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if unique.insert(trimmed.to_string()) {
            normalized.push(trimmed.to_string());
        }
        if let Some(stripped) = trimmed.strip_prefix(scoped_prefix.as_str())
            && !stripped.is_empty()
            && unique.insert(stripped.to_string())
        {
            normalized.push(stripped.to_string());
        }
    }
    normalized
}

fn merge_ingest_extra(existing: Option<Value>, requested_ingest_id: &str) -> Value {
    let mut object = match existing {
        Some(Value::Object(map)) => map,
        Some(value) => {
            let mut map = serde_json::Map::new();
            map.insert("value".to_string(), value);
            map
        }
        None => serde_json::Map::new(),
    };
    object.insert(
        "requested_ingest_id".to_string(),
        Value::String(requested_ingest_id.to_string()),
    );
    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use docx_store::models::{DocSource, Ingest, Project, RelationRecord, Symbol};
    use docx_store::schema::REL_MEMBER_OF;
    use serde::Deserialize;
    use surrealdb::Surreal;
    use surrealdb::engine::local::{Db, Mem};

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

    #[test]
    fn split_optional_doc_block_fts_schema_extracts_optional_block() {
        let schema = "\
DEFINE TABLE test SCHEMAFULL;\n\
-- OPTIONAL_DOC_BLOCK_FTS_START\n\
DEFINE ANALYZER IF NOT EXISTS docx_search TOKENIZERS blank,class FILTERS lowercase,snowball(english);\n\
-- OPTIONAL_DOC_BLOCK_FTS_END\n\
DEFINE INDEX test_idx ON TABLE test COLUMNS id;\n";
        let (required, optional) =
            split_optional_doc_block_fts_schema(schema).expect("split should succeed");
        let optional = optional.expect("optional block should be extracted");
        assert!(required.contains("DEFINE TABLE test SCHEMAFULL;"));
        assert!(required.contains("DEFINE INDEX test_idx ON TABLE test COLUMNS id;"));
        assert!(!required.contains("DEFINE ANALYZER"));
        assert!(optional.contains("DEFINE ANALYZER IF NOT EXISTS docx_search"));
    }

    #[test]
    fn split_optional_doc_block_fts_schema_rejects_unclosed_optional_block() {
        let schema = "\
DEFINE TABLE test SCHEMAFULL;\n\
-- OPTIONAL_DOC_BLOCK_FTS_START\n\
DEFINE ANALYZER IF NOT EXISTS docx_search TOKENIZERS blank,class FILTERS lowercase,snowball(english);\n";
        let error = split_optional_doc_block_fts_schema(schema)
            .expect_err("unclosed optional block should fail");
        assert!(
            error
                .to_string()
                .contains("start marker is missing a matching end marker"),
            "error should describe unmatched optional block markers"
        );
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
        assert_eq!(ingests[0].id.as_deref(), Some("project::ingest-1"));
        assert_eq!(
            ingests[0]
                .extra
                .as_ref()
                .and_then(|value| value.get("requested_ingest_id"))
                .and_then(serde_json::Value::as_str),
            Some("ingest-1")
        );
    }

    #[tokio::test]
    async fn list_ingests_scopes_same_ingest_id_per_project() {
        let store = build_store().await;
        let left = Ingest {
            id: Some("shared".to_string()),
            project_id: "project-left".to_string(),
            git_commit: None,
            git_branch: None,
            git_tag: None,
            project_version: None,
            source_modified_at: None,
            ingested_at: None,
            extra: None,
        };
        let right = Ingest {
            id: Some("shared".to_string()),
            project_id: "project-right".to_string(),
            git_commit: None,
            git_branch: None,
            git_tag: None,
            project_version: None,
            source_modified_at: None,
            ingested_at: None,
            extra: None,
        };

        store
            .create_ingest(left)
            .await
            .expect("failed to create left ingest");
        store
            .create_ingest(right)
            .await
            .expect("failed to create right ingest");

        let left_ingests = store
            .list_ingests("project-left", 10)
            .await
            .expect("failed to list left ingests");
        let right_ingests = store
            .list_ingests("project-right", 10)
            .await
            .expect("failed to list right ingests");

        assert_eq!(left_ingests.len(), 1);
        assert_eq!(right_ingests.len(), 1);
        assert_eq!(left_ingests[0].id.as_deref(), Some("project-left::shared"));
        assert_eq!(
            right_ingests[0].id.as_deref(),
            Some("project-right::shared")
        );
    }

    #[tokio::test]
    async fn get_ingest_supports_requested_id_when_unique() {
        let store = build_store().await;
        let ingest = Ingest {
            id: Some("requested".to_string()),
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
        let found = store
            .get_ingest("requested")
            .await
            .expect("failed to lookup ingest by requested id");
        assert_eq!(
            found.and_then(|row| row.id),
            Some("project::requested".to_string())
        );
    }

    #[tokio::test]
    async fn get_ingest_rejects_ambiguous_requested_id() {
        let store = build_store().await;
        let left = Ingest {
            id: Some("requested".to_string()),
            project_id: "project-left".to_string(),
            git_commit: None,
            git_branch: None,
            git_tag: None,
            project_version: None,
            source_modified_at: None,
            ingested_at: None,
            extra: None,
        };
        let right = Ingest {
            id: Some("requested".to_string()),
            project_id: "project-right".to_string(),
            git_commit: None,
            git_branch: None,
            git_tag: None,
            project_version: None,
            source_modified_at: None,
            ingested_at: None,
            extra: None,
        };

        store
            .create_ingest(left)
            .await
            .expect("failed to create left ingest");
        store
            .create_ingest(right)
            .await
            .expect("failed to create right ingest");

        let error = store
            .get_ingest("requested")
            .await
            .expect_err("ambiguous requested id should error");
        assert!(
            error
                .to_string()
                .contains("ambiguous; use project-scoped ingest id"),
            "error should explain scoped ingest id requirement"
        );
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

    #[tokio::test]
    async fn list_doc_sources_accepts_scoped_ingest_filter() {
        let store = build_store().await;
        let source = DocSource {
            id: Some("source-1".to_string()),
            project_id: "project".to_string(),
            ingest_id: Some("shared".to_string()),
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
            .list_doc_sources("project", &["project::shared".to_string()])
            .await
            .expect("failed to list doc sources with scoped ingest id");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id.as_deref(), Some("source-1"));
    }

    #[tokio::test]
    async fn list_doc_sources_by_project_accepts_scoped_ingest_filter() {
        let store = build_store().await;
        let source = DocSource {
            id: Some("source-1".to_string()),
            project_id: "project".to_string(),
            ingest_id: Some("shared".to_string()),
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
            .list_doc_sources_by_project("project", Some("project::shared"), 10)
            .await
            .expect("failed to list doc sources by project with scoped ingest id");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id.as_deref(), Some("source-1"));
    }

    #[tokio::test]
    async fn list_doc_sources_by_ids_filters_requested_ids() {
        let store = build_store().await;
        let source_left = DocSource {
            id: Some("source-left".to_string()),
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
        let source_right = DocSource {
            id: Some("source-right".to_string()),
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
            .create_doc_source(source_left)
            .await
            .expect("failed to create left doc source");
        store
            .create_doc_source(source_right)
            .await
            .expect("failed to create right doc source");

        let results = store
            .list_doc_sources_by_ids("project", &["source-right".to_string()])
            .await
            .expect("failed to list doc sources by ids");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id.as_deref(), Some("source-right"));
    }

    fn build_symbol(project_id: &str, id: &str) -> Symbol {
        Symbol {
            id: Some(id.to_string()),
            project_id: project_id.to_string(),
            language: Some("rust".to_string()),
            symbol_key: id.to_string(),
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

    #[derive(Deserialize, SurrealValue)]
    struct RelationTypeFlags {
        in_is_record: bool,
        out_is_record: bool,
    }

    #[tokio::test]
    async fn create_relation_stores_record_links() {
        let store = build_store().await;
        let _ = store
            .upsert_symbol(build_symbol("project", "left"))
            .await
            .expect("failed to create left symbol");
        let _ = store
            .upsert_symbol(build_symbol("project", "right"))
            .await
            .expect("failed to create right symbol");

        let relation = RelationRecord {
            id: None,
            in_id: "symbol:left".to_string(),
            out_id: "symbol:right".to_string(),
            project_id: "project".to_string(),
            ingest_id: None,
            kind: Some("test".to_string()),
            extra: None,
        };
        let _ = store
            .create_relation(REL_MEMBER_OF, relation)
            .await
            .expect("failed to create relation");

        let mut response = store
            .db()
            .query(
                "SELECT type::is_record(in) AS in_is_record, type::is_record(out) AS out_is_record FROM member_of LIMIT 1;",
            )
            .await
            .expect("failed to query relation record types");
        let rows: Vec<RelationTypeFlags> = response.take(0).expect("failed to decode relation row");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].in_is_record);
        assert!(rows[0].out_is_record);
    }

    #[tokio::test]
    async fn search_symbols_advanced_supports_exact_symbol_key() {
        let store = build_store().await;
        let mut alpha = build_symbol("project", "rust|project|alpha");
        alpha.name = Some("alpha".to_string());
        alpha.qualified_name = Some("crate::alpha".to_string());
        let mut beta = build_symbol("project", "rust|project|beta");
        beta.name = Some("beta".to_string());
        beta.qualified_name = Some("crate::beta".to_string());

        store
            .upsert_symbol(alpha.clone())
            .await
            .expect("failed to create alpha");
        store
            .upsert_symbol(beta)
            .await
            .expect("failed to create beta");

        let results = store
            .search_symbols_advanced(
                "project",
                None,
                None,
                Some(alpha.symbol_key.as_str()),
                None,
                10,
            )
            .await
            .expect("advanced search by symbol key should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].symbol_key, alpha.symbol_key);
    }

    #[tokio::test]
    async fn remove_database_makes_current_db_unavailable() {
        let store = build_store().await;
        let _ = store
            .upsert_project(Project {
                id: Some("project".to_string()),
                project_id: "project".to_string(),
                name: Some("project".to_string()),
                language: Some("rust".to_string()),
                root_path: None,
                description: None,
                aliases: Vec::new(),
                search_text: Some("project".to_string()),
                extra: None,
            })
            .await
            .expect("failed to upsert project");

        store
            .remove_database("test")
            .await
            .expect("failed to remove database");
        let result = store.list_projects(10).await;
        assert!(result.is_err());
    }
}
