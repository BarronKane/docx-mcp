//! Storage models and schema helpers for docx-mcp.
//!
//! This crate defines the canonical data model shared by parsers, control
//! plane, and storage backends.

pub mod models;
pub mod schema;

pub use models::*;
