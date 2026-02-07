use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ingest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_modified_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingested_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_modified_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Symbol {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub symbol_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_static: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_async: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_const: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_deprecated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TypeRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_params: Vec<TypeParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_ids: Vec<SourceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TypeRef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generics: Vec<TypeRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Param {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_ref: Option<TypeRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_optional: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TypeParam {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttributeRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceId {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<DocParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_params: Vec<DocTypeParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exceptions: Vec<DocException>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<DocExample>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panics: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub see_also: Vec<SeeAlso>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherit_doc: Option<DocInherit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<DocSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocParam {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_ref: Option<TypeRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocTypeParam {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocException {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_ref: Option<TypeRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocExample {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeeAlso {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocInherit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocSection {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocChunk {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_block_id: Option<String>,
    pub chunk_index: u32,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "in")]
    pub in_id: String,
    #[serde(rename = "out")]
    pub out_id: String,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

pub type ContainsEdge = RelationRecord;
pub type MemberOfEdge = RelationRecord;
pub type DocumentsEdge = RelationRecord;
pub type ReferencesEdge = RelationRecord;
pub type SeeAlsoEdge = RelationRecord;
pub type InheritsEdge = RelationRecord;
pub type ImplementsEdge = RelationRecord;
pub type OverloadOfEdge = RelationRecord;
pub type TypeOfEdge = RelationRecord;
pub type ReturnsEdge = RelationRecord;
pub type ParamTypeEdge = RelationRecord;
pub type ObservedInEdge = RelationRecord;
