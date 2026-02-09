//! MCP tool modules.
//!
//! Tools are grouped by domain: ingestion, metadata lookup, symbol/query data
//! access, and contextual help for ingestion workflows.
//!
//! The tool surface is intentionally verbose so that AI clients can discover
//! and request metadata-rich responses, including relation adjacency graphs.

pub mod ingest;
pub mod data;
pub mod metadata;
mod context;
