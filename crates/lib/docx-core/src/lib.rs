//! Core types and services for docx-mcp.
//!
//! This crate owns the ingestion pipeline for documentation sources, exposes
//! control-plane helpers for querying stored symbols, and provides the `SurrealDB`
//! backing store implementation.

pub mod control;
pub mod parsers;
pub mod services;
pub mod store;
