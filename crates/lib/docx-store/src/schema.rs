pub const TABLE_PROJECT: &str = "project";
pub const TABLE_INGEST: &str = "ingest";
pub const TABLE_DOC_SOURCE: &str = "doc_source";
pub const TABLE_SYMBOL: &str = "symbol";
pub const TABLE_DOC_BLOCK: &str = "doc_block";
pub const TABLE_DOC_CHUNK: &str = "doc_chunk";

pub const REL_CONTAINS: &str = "contains";
pub const REL_MEMBER_OF: &str = "member_of";
pub const REL_DOCUMENTS: &str = "documents";
pub const REL_REFERENCES: &str = "references";
pub const REL_SEE_ALSO: &str = "see_also";
pub const REL_INHERITS: &str = "inherits";
pub const REL_IMPLEMENTS: &str = "implements";
pub const REL_OVERLOAD_OF: &str = "overload_of";
pub const REL_TYPE_OF: &str = "type_of";
pub const REL_RETURNS: &str = "returns";
pub const REL_PARAM_TYPE: &str = "param_type";
pub const REL_OBSERVED_IN: &str = "observed_in";

pub const SOURCE_KIND_CSHARP_XML: &str = "csharp_xml";
pub const SOURCE_KIND_RUSTDOC_JSON: &str = "rustdoc_json";
pub const SOURCE_KIND_DOXYGEN_XML: &str = "doxygen_xml";

pub fn make_symbol_key(language: &str, project_id: &str, local_id: &str) -> String {
    format!("{language}|{project_id}|{local_id}")
}

pub fn make_csharp_symbol_key(project_id: &str, doc_id: &str) -> String {
    make_symbol_key("csharp", project_id, doc_id)
}
