//! Store interfaces and `SurrealDB` implementation.
//!
//! The store layer handles persistence of symbols, doc blocks, and relations.

pub mod surreal;

pub use surreal::{StoreError, StoreResult, SurrealDocStore};
