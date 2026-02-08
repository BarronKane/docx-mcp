//! Parsers for source documentation inputs.
//!
//! Each parser normalizes an external documentation format into symbols and
//! doc blocks suitable for the canonical data model.

pub mod csharp_xml;
pub mod rustdoc_json;

pub use csharp_xml::{
    CsharpParseError,
    CsharpParseOptions,
    CsharpParseOutput,
    CsharpXmlParser,
};
pub use rustdoc_json::{
    RustdocJsonParser,
    RustdocParseError,
    RustdocParseOptions,
    RustdocParseOutput,
};
