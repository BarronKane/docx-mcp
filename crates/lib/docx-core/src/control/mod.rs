use std::{error::Error, fmt, sync::Arc};

use surrealdb::{Connection, Surreal};

use crate::parsers::CsharpParseError;
use crate::store::{StoreError, SurrealDocStore};

pub mod ingest;
pub mod metadata;
pub mod data;

pub use ingest::{CsharpIngestReport, CsharpIngestRequest};
pub use metadata::ProjectUpsertRequest;

#[derive(Debug)]
pub enum ControlError {
    Parse(CsharpParseError),
    Store(StoreError),
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "{err}"),
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

impl From<StoreError> for ControlError {
    fn from(err: StoreError) -> Self {
        Self::Store(err)
    }
}

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
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            store: SurrealDocStore::new(db),
        }
    }

    pub fn from_arc(db: Arc<Surreal<C>>) -> Self {
        Self {
            store: SurrealDocStore::from_arc(db),
        }
    }

    pub fn with_store(store: SurrealDocStore<C>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &SurrealDocStore<C> {
        &self.store
    }
}
