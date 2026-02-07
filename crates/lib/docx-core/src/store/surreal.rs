use std::{error::Error, fmt, sync::Arc};

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

#[derive(Debug)]
pub enum StoreError {
    Surreal(surrealdb::Error),
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
        Self::Surreal(err)
    }
}

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Clone)]
pub struct SurrealDocStore<C: Connection> {
    db: Arc<Surreal<C>>,
}

impl<C: Connection> SurrealDocStore<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            db: Arc::new(db),
        }
    }

    pub fn from_arc(db: Arc<Surreal<C>>) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &Surreal<C> {
        &self.db
    }

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

    pub async fn create_ingest(&self, ingest: Ingest) -> StoreResult<Ingest> {
        let record: Ingest = self.db.create(TABLE_INGEST).content(ingest).await?;
        Ok(record)
    }

    pub async fn create_doc_source(&self, source: DocSource) -> StoreResult<DocSource> {
        let record: DocSource = self.db.create(TABLE_DOC_SOURCE).content(source).await?;
        Ok(record)
    }

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

    pub async fn create_doc_block(&self, block: DocBlock) -> StoreResult<DocBlock> {
        let record: DocBlock = self.db.create(TABLE_DOC_BLOCK).content(block).await?;
        Ok(record)
    }

    pub async fn create_doc_blocks(&self, blocks: Vec<DocBlock>) -> StoreResult<Vec<DocBlock>> {
        if blocks.is_empty() {
            return Ok(Vec::new());
        }
        let records: Vec<DocBlock> = self.db.create(TABLE_DOC_BLOCK).content(blocks).await?;
        Ok(records)
    }

    pub async fn create_doc_chunks(&self, chunks: Vec<DocChunk>) -> StoreResult<Vec<DocChunk>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }
        let records: Vec<DocChunk> = self.db.create(TABLE_DOC_CHUNK).content(chunks).await?;
        Ok(records)
    }

    pub async fn create_relation(
        &self,
        table: &str,
        relation: RelationRecord,
    ) -> StoreResult<RelationRecord> {
        let record: RelationRecord = self.db.create(table).content(relation).await?;
        Ok(record)
    }

    pub async fn create_relations(
        &self,
        table: &str,
        relations: Vec<RelationRecord>,
    ) -> StoreResult<Vec<RelationRecord>> {
        if relations.is_empty() {
            return Ok(Vec::new());
        }
        let records: Vec<RelationRecord> = self.db.create(table).content(relations).await?;
        Ok(records)
    }

    pub async fn get_symbol(&self, symbol_key: &str) -> StoreResult<Option<Symbol>> {
        let record: Option<Symbol> = self.db.select((TABLE_SYMBOL, symbol_key)).await?;
        Ok(record)
    }

    pub async fn get_symbol_by_project(
        &self,
        project_id: &str,
        symbol_key: &str,
    ) -> StoreResult<Option<Symbol>> {
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

    pub async fn list_symbols_by_name(
        &self,
        project_id: &str,
        name: &str,
        limit: usize,
    ) -> StoreResult<Vec<Symbol>> {
        let query = "SELECT * FROM symbol WHERE project_id = $project_id AND name CONTAINS $name LIMIT $limit;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("name", name))
            .bind(("limit", limit as i64))
            .await?;
        let records: Vec<Symbol> = response.take(0)?;
        Ok(records)
    }

    pub async fn list_doc_blocks(
        &self,
        project_id: &str,
        symbol_key: &str,
        ingest_id: Option<&str>,
    ) -> StoreResult<Vec<DocBlock>> {
        let (query, binds) = if let Some(ingest_id) = ingest_id {
            (
                "SELECT * FROM doc_block WHERE project_id = $project_id AND symbol_key = $symbol_key AND ingest_id = $ingest_id;",
                Some(ingest_id),
            )
        } else {
            (
                "SELECT * FROM doc_block WHERE project_id = $project_id AND symbol_key = $symbol_key;",
                None,
            )
        };
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("symbol_key", symbol_key));
        let response = if let Some(ingest_id) = binds {
            response.bind(("ingest_id", ingest_id)).await?
        } else {
            response.await?
        };
        let records: Vec<DocBlock> = response.take(0)?;
        Ok(records)
    }

    pub async fn search_doc_blocks(
        &self,
        project_id: &str,
        text: &str,
        limit: usize,
    ) -> StoreResult<Vec<DocBlock>> {
        let query = "SELECT * FROM doc_block WHERE project_id = $project_id AND (summary CONTAINS $text OR remarks CONTAINS $text OR returns CONTAINS $text) LIMIT $limit;";
        let mut response = self
            .db
            .query(query)
            .bind(("project_id", project_id))
            .bind(("text", text))
            .bind(("limit", limit as i64))
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
