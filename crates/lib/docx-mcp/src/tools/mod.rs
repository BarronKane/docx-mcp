//! MCP tool modules.
//!
//! Tools are grouped by domain: ingestion, metadata lookup, symbol/query data
//! access, and contextual help for ingestion workflows.
//!
//! The tool surface is intentionally verbose so that AI clients can discover
//! and request metadata-rich responses, including relation adjacency graphs.

mod context;
pub mod data;
pub mod ingest;
pub mod metadata;
