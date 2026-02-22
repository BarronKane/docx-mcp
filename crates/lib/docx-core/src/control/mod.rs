//! Control-plane operations and requests for docx-mcp.
//!
//! The control plane coordinates parsing, ingestion, and query operations for
//! a single solution against the backing store.

use std::{error::Error, fmt, sync::Arc};

use surrealdb::{Connection, Surreal};

use crate::parsers::{CsharpParseError, RustdocParseError};
use crate::store::{StoreError, SurrealDocStore};

pub mod data;
pub mod ingest;
pub mod metadata;

pub use ingest::{CsharpIngestReport, CsharpIngestRequest};
pub use ingest::{RustdocIngestReport, RustdocIngestRequest};
pub use metadata::ProjectUpsertRequest;

/// Errors returned by control-plane operations.
#[derive(Debug)]
pub enum ControlError {
    /// C# XML parse error.
    Parse(CsharpParseError),
    /// Rustdoc JSON parse error.
    RustdocParse(RustdocParseError),
    Store(StoreError),
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "{err}"),
            Self::RustdocParse(err) => write!(f, "{err}"),
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

impl From<RustdocParseError> for ControlError {
    fn from(err: RustdocParseError) -> Self {
        Self::RustdocParse(err)
    }
}

impl From<StoreError> for ControlError {
    fn from(err: StoreError) -> Self {
        Self::Store(err)
    }
}

/// Facade for ingestion and query operations for a single solution store.
pub struct DocxControlPlane<C: Connection> {
    store: SurrealDocStore<C>,
}

impl<C: Connection> Clone for DocxControlPlane<C> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
        }
    }
}

impl<C: Connection> DocxControlPlane<C> {
    /// Creates a control plane from a `SurrealDB` connection.
    #[must_use]
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            store: SurrealDocStore::new(db),
        }
    }

    /// Creates a control plane from a shared `SurrealDB` connection.
    #[must_use]
    pub fn from_arc(db: Arc<Surreal<C>>) -> Self {
        Self {
            store: SurrealDocStore::from_arc(db),
        }
    }

    /// Creates a control plane from an existing store implementation.
    #[must_use]
    pub const fn with_store(store: SurrealDocStore<C>) -> Self {
        Self { store }
    }

    /// Returns the underlying store implementation.
    #[must_use]
    pub const fn store(&self) -> &SurrealDocStore<C> {
        &self.store
    }
}
