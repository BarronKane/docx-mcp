//! Rustdoc JSON parser.

use std::collections::{HashMap, HashSet};
use std::{error::Error, fmt, path::Path};

use docx_store::models::{
    DocBlock,
    DocExample,
    DocParam,
    DocSection,
    DocTypeParam,
    Param,
    SeeAlso,
    SourceId,
    Symbol,
    TypeParam,
    TypeRef,
};
use docx_store::schema::{SOURCE_KIND_RUSTDOC_JSON, make_symbol_key};
use serde::Deserialize;
use serde_json::Value;

/// Options for parsing rustdoc JSON.
#[derive(Debug, Clone)]
pub struct RustdocParseOptions {
    pub project_id: String,
    pub ingest_id: Option<String>,
    pub language: String,
    pub source_kind: String,
}

impl RustdocParseOptions {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            ingest_id: None,
            language: "rust".to_string(),
            source_kind: SOURCE_KIND_RUSTDOC_JSON.to_string(),
        }
    }

    #[must_use]
    pub fn with_ingest_id(mut self, ingest_id: impl Into<String>) -> Self {
        self.ingest_id = Some(ingest_id.into());
        self
    }
}

/// Output from parsing rustdoc JSON.
#[derive(Debug, Clone)]
pub struct RustdocParseOutput {
    pub crate_name: Option<String>,
    pub symbols: Vec<Symbol>,
    pub doc_blocks: Vec<DocBlock>,
}

/// Error type for rustdoc JSON parse failures.
#[derive(Debug)]
pub struct RustdocParseError {
    message: String,
}

impl RustdocParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RustdocParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rustdoc JSON parse error: {}", self.message)
    }
}

impl Error for RustdocParseError {}

impl From<serde_json::Error> for RustdocParseError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(err.to_string())
    }
}

impl From<std::io::Error> for RustdocParseError {
    fn from(err: std::io::Error) -> Self {
        Self::new(err.to_string())
    }
}

impl From<tokio::task::JoinError> for RustdocParseError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::new(err.to_string())
    }
}

/// Parser for rustdoc JSON output.
pub struct RustdocJsonParser;

impl RustdocJsonParser {
    /// Parses rustdoc JSON into symbols and doc blocks.
    ///
    /// # Errors
    /// Returns `RustdocParseError` if the JSON is invalid or cannot be parsed.
    #[allow(clippy::too_many_lines)]
    pub fn parse(
        json: &str,
        options: &RustdocParseOptions,
    ) -> Result<RustdocParseOutput, RustdocParseError> {
        let crate_doc: RustdocCrate = serde_json::from_str(json)?;
        let root_id = crate_doc.root;
        let root_item = crate_doc
            .index
            .get(&root_id.to_string())
            .ok_or_else(|| RustdocParseError::new("missing root item"))?;

        let crate_name = root_item.name.clone();
        let root_crate_id = root_item.crate_id;
        let mut id_to_path = build_id_path_map(&crate_doc, root_crate_id);

        let mut state = ParserState {
            crate_doc: &crate_doc,
            options,
            root_crate_id,
            id_to_path: &mut id_to_path,
            symbols: Vec::new(),
            doc_blocks: Vec::new(),
            seen: HashSet::new(),
        };

        let mut module_path = Vec::new();
        if let Some(name) = crate_name.clone() {
            module_path.push(name);
        }
        state.visit_module(root_id, &module_path);

        Ok(RustdocParseOutput {
            crate_name,
            symbols: state.symbols,
            doc_blocks: state.doc_blocks,
        })
    }
    /// Parses rustdoc JSON asynchronously using a blocking task.
    ///
    /// # Errors
    /// Returns `RustdocParseError` if parsing fails or the task panics.
    pub async fn parse_async(
        json: String,
        options: RustdocParseOptions,
    ) -> Result<RustdocParseOutput, RustdocParseError> {
        tokio::task::spawn_blocking(move || Self::parse(&json, &options)).await?
    }

    /// Parses rustdoc JSON from a file path asynchronously.
    ///
    /// # Errors
    /// Returns `RustdocParseError` if the file cannot be read or the JSON cannot be parsed.
    pub async fn parse_file(
        path: impl AsRef<Path>,
        options: RustdocParseOptions,
    ) -> Result<RustdocParseOutput, RustdocParseError> {
        let path = path.as_ref().to_path_buf();
        let json = tokio::task::spawn_blocking(move || std::fs::read_to_string(path)).await??;
        Self::parse_async(json, options).await
    }
}

#[derive(Debug, Deserialize)]
struct RustdocCrate {
    root: u64,
    index: HashMap<String, RustdocItem>,
    #[serde(default)]
    paths: HashMap<String, RustdocPath>,
}

#[derive(Debug, Deserialize, Clone)]
struct RustdocItem {
    id: u64,
    crate_id: u64,
    name: Option<String>,
    span: Option<RustdocSpan>,
    visibility: Option<String>,
    docs: Option<String>,
    deprecation: Option<RustdocDeprecation>,
    inner: HashMap<String, Value>,
}

#[derive(Debug, Deserialize, Clone)]
struct RustdocSpan {
    filename: String,
    begin: [u32; 2],
}

#[derive(Debug, Deserialize, Clone)]
struct RustdocPath {
    crate_id: u64,
    path: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct RustdocDeprecation {
    since: Option<String>,
}

struct ParserState<'a> {
    crate_doc: &'a RustdocCrate,
    options: &'a RustdocParseOptions,
    root_crate_id: u64,
    id_to_path: &'a mut HashMap<u64, String>,
    symbols: Vec<Symbol>,
    doc_blocks: Vec<DocBlock>,
    seen: HashSet<u64>,
}
impl ParserState<'_> {
    fn visit_module(&mut self, module_id: u64, module_path: &[String]) {
        if self.seen.contains(&module_id) {
            return;
        }
        let Some(item) = self.get_item(module_id) else {
            return;
        };
        if item.crate_id != self.root_crate_id {
            return;
        }
        self.seen.insert(module_id);

        self.add_symbol(&item, module_path, None, Some("module"));
        let items = module_items(&item);
        for child_id in items {
            if let Some(child) = self.get_item(child_id) {
                if child.crate_id != self.root_crate_id {
                    continue;
                }
                if is_inner_kind(&child, "module") {
                    let mut child_path = module_path.to_vec();
                    if let Some(name) = child.name.as_ref() && !name.is_empty() {
                        child_path.push(name.clone());
                    }
                    self.visit_module(child_id, &child_path);
                } else {
                    self.visit_item(child_id, module_path);
                }
            }
        }
    }

    fn visit_item(&mut self, item_id: u64, module_path: &[String]) {
        if self.seen.contains(&item_id) {
            return;
        }
        let Some(item) = self.get_item(item_id) else {
            return;
        };
        if item.crate_id != self.root_crate_id {
            return;
        }
        self.seen.insert(item_id);

        let inner_kind = inner_kind(&item);
        match inner_kind {
            Some("struct") => {
                let qualified = self.add_symbol(&item, module_path, None, Some("struct"));
                self.visit_struct_fields(&item, &qualified);
                self.visit_impls(&item, &qualified);
            }
            Some("enum") => {
                let qualified = self.add_symbol(&item, module_path, None, Some("enum"));
                self.visit_enum_variants(&item, &qualified);
                self.visit_impls(&item, &qualified);
            }
            Some("trait") => {
                let qualified = self.add_symbol(&item, module_path, None, Some("trait"));
                self.visit_trait_items(&item, &qualified);
                self.visit_impls(&item, &qualified);
            }
            Some("function") => {
                self.add_symbol(&item, module_path, None, Some("function"));
            }
            Some("type_alias") => {
                self.add_symbol(&item, module_path, None, Some("type_alias"));
            }
            Some("constant") => {
                self.add_symbol(&item, module_path, None, Some("const"));
            }
            Some("static") => {
                self.add_symbol(&item, module_path, None, Some("static"));
            }
            Some("union") => {
                self.add_symbol(&item, module_path, None, Some("union"));
            }
            Some("macro") => {
                self.add_symbol(&item, module_path, None, Some("macro"));
            }
            Some("module") => {
                let mut child_path = module_path.to_vec();
                if let Some(name) = item.name.as_ref() && !name.is_empty() {
                    child_path.push(name.clone());
                }
                self.visit_module(item_id, &child_path);
            }
            _ => {}
        }
    }

    fn visit_struct_fields(&mut self, item: &RustdocItem, owner_name: &str) {
        let Some(inner) = item.inner.get("struct") else {
            return;
        };
        let Some(kind) = inner.get("kind") else {
            return;
        };
        let field_ids = struct_kind_fields(kind);
        for field_id in field_ids {
            if let Some(field_item) = self.get_item(field_id) {
                if field_item.crate_id != self.root_crate_id {
                    continue;
                }
                self.add_symbol(&field_item, &[], Some(owner_name), Some("field"));
            }
        }
    }

    fn visit_enum_variants(&mut self, item: &RustdocItem, owner_name: &str) {
        let Some(inner) = item.inner.get("enum") else {
            return;
        };
        let Some(variants) = inner.get("variants").and_then(Value::as_array) else {
            return;
        };
        for variant_id in variants.iter().filter_map(Value::as_u64) {
            if let Some(variant_item) = self.get_item(variant_id) {
                if variant_item.crate_id != self.root_crate_id {
                    continue;
                }
                self.add_symbol(&variant_item, &[], Some(owner_name), Some("variant"));
            }
        }
    }

    fn visit_trait_items(&mut self, item: &RustdocItem, owner_name: &str) {
        let Some(inner) = item.inner.get("trait") else {
            return;
        };
        let Some(items) = inner.get("items").and_then(Value::as_array) else {
            return;
        };
        for assoc_id in items.iter().filter_map(Value::as_u64) {
            if let Some(assoc_item) = self.get_item(assoc_id) {
                if assoc_item.crate_id != self.root_crate_id {
                    continue;
                }
                self.add_symbol(&assoc_item, &[], Some(owner_name), Some("trait_item"));
            }
        }
    }

    fn visit_impls(&mut self, item: &RustdocItem, owner_name: &str) {
        let impl_ids = match inner_kind(item) {
            Some("struct") => item
                .inner
                .get("struct")
                .and_then(|value| value.get("impls"))
                .and_then(Value::as_array)
                .map(|items| extract_ids(items)),
            Some("enum") => item
                .inner
                .get("enum")
                .and_then(|value| value.get("impls"))
                .and_then(Value::as_array)
                .map(|items| extract_ids(items)),
            Some("trait") => item
                .inner
                .get("trait")
                .and_then(|value| value.get("impls"))
                .and_then(Value::as_array)
                .map(|items| extract_ids(items)),
            _ => None,
        };

        let Some(impl_ids) = impl_ids else {
            return;
        };

        for impl_id in impl_ids {
            let Some(impl_item) = self.get_item(impl_id) else {
                continue;
            };
            if impl_item.crate_id != self.root_crate_id {
                continue;
            }
            let Some(impl_inner) = impl_item.inner.get("impl") else {
                continue;
            };
            let Some(items) = impl_inner.get("items").and_then(Value::as_array) else {
                continue;
            };
            for assoc_id in items.iter().filter_map(Value::as_u64) {
                if let Some(assoc_item) = self.get_item(assoc_id) {
                    if assoc_item.crate_id != self.root_crate_id {
                        continue;
                    }
                    self.add_symbol(&assoc_item, &[], Some(owner_name), Some("method"));
                }
            }
        }
    }

    fn add_symbol(
        &mut self,
        item: &RustdocItem,
        module_path: &[String],
        owner_name: Option<&str>,
        kind_override: Option<&str>,
    ) -> String {
        let name = item.name.clone().unwrap_or_default();
        let qualified_name = qualified_name_for_item(&name, module_path, owner_name);

        let symbol_key = make_symbol_key("rust", &self.options.project_id, &qualified_name);
        let doc_symbol_key = symbol_key.clone();
        self.id_to_path.insert(item.id, qualified_name.clone());

        let docs = item.docs.as_deref().unwrap_or("").trim();
        let parsed_docs = (!docs.is_empty()).then(|| parse_markdown_docs(docs));

        let (params, return_type, signature) = parse_signature(item, self, &name);
        let type_params = parse_type_params(item);
        let (source_path, line, col) = span_location(item);

        let parts = SymbolParts {
            name,
            qualified_name: qualified_name.clone(),
            symbol_key,
            signature,
            params,
            return_type,
            type_params,
            source_path,
            line,
            col,
        };

        let symbol = build_symbol(item, self.options, parts, kind_override, parsed_docs.as_ref());
        self.symbols.push(symbol);

        if let Some(parsed_docs) = parsed_docs {
            let doc_block = build_doc_block(self.options, doc_symbol_key, parsed_docs, docs);
            self.doc_blocks.push(doc_block);
        }

        qualified_name
    }

    fn get_item(&self, item_id: u64) -> Option<RustdocItem> {
        self.crate_doc
            .index
            .get(&item_id.to_string())
            .cloned()
    }
}

fn qualified_name_for_item(
    name: &str,
    module_path: &[String],
    owner_name: Option<&str>,
) -> String {
    owner_name.map_or_else(
        || {
            if module_path.is_empty() {
                name.to_string()
            } else if name.is_empty() {
                module_path.join("::")
            } else {
                format!("{}::{name}", module_path.join("::"))
            }
        },
        |owner| {
            if name.is_empty() {
                owner.to_string()
            } else {
                format!("{owner}::{name}")
            }
        },
    )
}

fn span_location(item: &RustdocItem) -> (Option<String>, Option<u32>, Option<u32>) {
    item.span.as_ref().map_or((None, None, None), |span| {
        (
            Some(span.filename.clone()),
            Some(span.begin[0]),
            Some(span.begin[1]),
        )
    })
}

struct SymbolParts {
    name: String,
    qualified_name: String,
    symbol_key: String,
    signature: Option<String>,
    params: Vec<Param>,
    return_type: Option<TypeRef>,
    type_params: Vec<TypeParam>,
    source_path: Option<String>,
    line: Option<u32>,
    col: Option<u32>,
}

fn build_symbol(
    item: &RustdocItem,
    options: &RustdocParseOptions,
    parts: SymbolParts,
    kind_override: Option<&str>,
    parsed_docs: Option<&ParsedDocs>,
) -> Symbol {
    let SymbolParts {
        name,
        qualified_name,
        symbol_key,
        signature,
        params,
        return_type,
        type_params,
        source_path,
        line,
        col,
    } = parts;

    let name_value = if name.is_empty() {
        None
    } else {
        Some(name)
    };
    let qualified_value = if qualified_name.is_empty() {
        None
    } else {
        Some(qualified_name)
    };

    Symbol {
        id: None,
        project_id: options.project_id.clone(),
        language: Some(options.language.clone()),
        symbol_key,
        kind: kind_override.map(str::to_string).or_else(|| inner_kind(item).map(str::to_string)),
        name: name_value.clone(),
        qualified_name: qualified_value,
        display_name: name_value,
        signature,
        signature_hash: None,
        visibility: item.visibility.clone(),
        is_static: item_is_static(item),
        is_async: item_is_async(item),
        is_const: item_is_const(item),
        is_deprecated: item.deprecation.is_some().then_some(true),
        since: item.deprecation.as_ref().and_then(|dep| dep.since.clone()),
        stability: None,
        source_path,
        line,
        col,
        return_type,
        params,
        type_params,
        attributes: Vec::new(),
        source_ids: vec![SourceId {
            kind: "rustdoc_id".to_string(),
            value: item.id.to_string(),
        }],
        doc_summary: parsed_docs.and_then(|docs| docs.summary.clone()),
        extra: None,
    }
}

fn build_doc_block(
    options: &RustdocParseOptions,
    symbol_key: String,
    parsed_docs: ParsedDocs,
    raw_docs: &str,
) -> DocBlock {
    DocBlock {
        id: None,
        project_id: options.project_id.clone(),
        ingest_id: options.ingest_id.clone(),
        symbol_key: Some(symbol_key),
        language: Some(options.language.clone()),
        source_kind: Some(options.source_kind.clone()),
        doc_hash: None,
        summary: parsed_docs.summary,
        remarks: parsed_docs.remarks,
        returns: parsed_docs.returns,
        value: parsed_docs.value,
        params: parsed_docs.params,
        type_params: parsed_docs.type_params,
        exceptions: Vec::new(),
        examples: parsed_docs.examples,
        notes: parsed_docs.notes,
        warnings: parsed_docs.warnings,
        safety: parsed_docs.safety,
        panics: parsed_docs.panics,
        errors: parsed_docs.errors,
        see_also: parsed_docs.see_also,
        deprecated: parsed_docs.deprecated,
        inherit_doc: None,
        sections: parsed_docs.sections,
        raw: Some(raw_docs.to_string()),
        extra: None,
    }
}

#[derive(Debug)]
struct ParsedDocs {
    summary: Option<String>,
    remarks: Option<String>,
    returns: Option<String>,
    value: Option<String>,
    errors: Option<String>,
    panics: Option<String>,
    safety: Option<String>,
    deprecated: Option<String>,
    params: Vec<DocParam>,
    type_params: Vec<DocTypeParam>,
    examples: Vec<DocExample>,
    notes: Vec<String>,
    warnings: Vec<String>,
    see_also: Vec<SeeAlso>,
    sections: Vec<DocSection>,
}

fn build_id_path_map(crate_doc: &RustdocCrate, root_crate_id: u64) -> HashMap<u64, String> {
    let mut map = HashMap::new();
    for (id, path) in &crate_doc.paths {
        if path.crate_id != root_crate_id {
            continue;
        }
        if let Ok(parsed_id) = id.parse::<u64>() {
            let joined = path.path.join("::");
            map.insert(parsed_id, joined);
        }
    }
    map
}

fn inner_kind(item: &RustdocItem) -> Option<&str> {
    item.inner.keys().next().map(String::as_str)
}

fn is_inner_kind(item: &RustdocItem, kind: &str) -> bool {
    matches!(inner_kind(item), Some(found) if found == kind)
}

fn module_items(item: &RustdocItem) -> Vec<u64> {
    item.inner
        .get("module")
        .and_then(|value| value.get("items"))
        .and_then(Value::as_array)
        .map(|items| extract_ids(items))
        .unwrap_or_default()
}

fn struct_kind_fields(kind: &Value) -> Vec<u64> {
    if let Some(plain) = kind.get("plain") {
        return plain
            .get("fields")
            .and_then(Value::as_array)
            .map(|items| extract_ids(items))
            .unwrap_or_default();
    }
    if let Some(tuple) = kind.get("tuple") {
        return tuple
            .get("fields")
            .and_then(Value::as_array)
            .map(|items| extract_ids(items))
            .unwrap_or_default();
    }
    Vec::new()
}

fn extract_ids(items: &[Value]) -> Vec<u64> {
    items.iter().filter_map(Value::as_u64).collect()
}

fn parse_signature(
    item: &RustdocItem,
    state: &ParserState<'_>,
    name: &str,
) -> (Vec<Param>, Option<TypeRef>, Option<String>) {
    let Some(inner) = item.inner.get("function") else {
        let return_type = match inner_kind(item) {
            Some("constant") => item
                .inner
                .get("constant")
                .and_then(|value| value.get("type"))
                .map(|ty| type_to_ref(ty, state)),
            Some("static") => item
                .inner
                .get("static")
                .and_then(|value| value.get("type"))
                .map(|ty| type_to_ref(ty, state)),
            Some("struct_field") => item
                .inner
                .get("struct_field")
                .map(|ty| type_to_ref(ty, state)),
            Some("type_alias") => item
                .inner
                .get("type_alias")
                .and_then(|value| value.get("type"))
                .map(|ty| type_to_ref(ty, state)),
            _ => None,
        };
        return (Vec::new(), return_type, None);
    };

    let Some(sig) = inner.get("sig") else {
        return (Vec::new(), None, None);
    };

    let mut params = Vec::new();
    if let Some(inputs) = sig.get("inputs").and_then(Value::as_array) {
        for input in inputs {
            let Some(pair) = input.as_array() else {
                continue;
            };
            if pair.len() != 2 {
                continue;
            }
            let name = pair[0].as_str().unwrap_or("").to_string();
            let ty = type_to_ref(&pair[1], state);
            params.push(Param {
                name,
                type_ref: Some(ty),
                default_value: None,
                is_optional: None,
            });
        }
    }

    let return_type = sig
        .get("output")
        .and_then(|output| {
            if output.is_null() {
                None
            } else {
                Some(type_to_ref(output, state))
            }
        });

    let signature = format_function_signature(name, &params, return_type.as_ref());
    (params, return_type, Some(signature))
}

fn parse_type_params(item: &RustdocItem) -> Vec<TypeParam> {
    let Some(kind) = inner_kind(item) else {
        return Vec::new();
    };
    let generics = match kind {
        "function" => item
            .inner
            .get("function")
            .and_then(|value| value.get("generics")),
        "struct" => item
            .inner
            .get("struct")
            .and_then(|value| value.get("generics")),
        "enum" => item.inner.get("enum").and_then(|value| value.get("generics")),
        "trait" => item.inner.get("trait").and_then(|value| value.get("generics")),
        "type_alias" => item
            .inner
            .get("type_alias")
            .and_then(|value| value.get("generics")),
        _ => None,
    };

    let Some(generics) = generics else {
        return Vec::new();
    };
    let Some(params) = generics.get("params").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut output = Vec::new();
    for param in params {
        let Some(name) = param.get("name").and_then(Value::as_str) else {
            continue;
        };
        let mut constraints = Vec::new();
        if let Some(bounds) = param
            .get("kind")
            .and_then(|kind| kind.get("type"))
            .and_then(|type_info| type_info.get("bounds"))
            .and_then(Value::as_array)
        {
            for bound in bounds {
                if let Some(path) = bound
                    .get("trait_bound")
                    .and_then(|trait_bound| trait_bound.get("trait"))
                    .and_then(|trait_path| trait_path.get("path"))
                    .and_then(Value::as_str)
                {
                    constraints.push(path.to_string());
                }
            }
        }
        output.push(TypeParam {
            name: name.to_string(),
            constraints,
        });
    }
    output
}

fn item_is_async(item: &RustdocItem) -> Option<bool> {
    item.inner
        .get("function")
        .and_then(|value| value.get("header"))
        .and_then(|value| value.get("is_async"))
        .and_then(Value::as_bool)
        .filter(|is_async| *is_async)
        .map(|_| true)
}

fn item_is_const(item: &RustdocItem) -> Option<bool> {
    if matches!(inner_kind(item), Some("constant")) {
        return Some(true);
    }
    item.inner
        .get("function")
        .and_then(|value| value.get("header"))
        .and_then(|value| value.get("is_const"))
        .and_then(Value::as_bool)
        .filter(|is_const| *is_const)
        .map(|_| true)
}

fn item_is_static(item: &RustdocItem) -> Option<bool> {
    matches!(inner_kind(item), Some("static")).then_some(true)
}
fn format_function_signature(
    name: &str,
    params: &[Param],
    output: Option<&TypeRef>,
) -> String {
    let params = params
        .iter()
        .map(|param| match param.type_ref.as_ref().and_then(|ty| ty.display.as_ref()) {
            Some(ty) if !param.name.is_empty() => format!("{}: {ty}", param.name),
            Some(ty) => ty.clone(),
            None => param.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut sig = format!("fn {name}({params})");
    if let Some(output) = output.and_then(|ty| ty.display.as_ref()) && output != "()" {
        sig.push_str(" -> ");
        sig.push_str(output);
    }
    sig
}

fn type_to_ref(value: &Value, state: &ParserState<'_>) -> TypeRef {
    let display = type_to_string(value, state).unwrap_or_else(|| "<unknown>".to_string());
    let symbol_key = type_symbol_key(value, state);
    TypeRef {
        display: Some(display.clone()),
        canonical: Some(display),
        language: Some(state.options.language.clone()),
        symbol_key,
        generics: Vec::new(),
        modifiers: Vec::new(),
    }
}

fn type_symbol_key(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let resolved = value.get("resolved_path")?;
    let id = resolved.get("id").and_then(Value::as_u64)?;
    let path = state.id_to_path.get(&id)?.clone();
    Some(make_symbol_key("rust", &state.options.project_id, &path))
}

fn type_to_string(value: &Value, state: &ParserState<'_>) -> Option<String> {
    primitive_type(value)
        .or_else(|| generic_type(value))
        .or_else(|| resolved_path_type(value, state))
        .or_else(|| borrowed_ref_type(value, state))
        .or_else(|| raw_pointer_type(value, state))
        .or_else(|| tuple_type(value, state))
        .or_else(|| slice_type(value, state))
        .or_else(|| array_type(value, state))
        .or_else(|| impl_trait_type(value, state))
        .or_else(|| dyn_trait_type(value, state))
        .or_else(|| qualified_path_type(value, state))
        .or_else(|| function_pointer_type(value, state))
}

fn primitive_type(value: &Value) -> Option<String> {
    value
        .get("primitive")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn generic_type(value: &Value) -> Option<String> {
    value
        .get("generic")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn resolved_path_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let resolved = value.get("resolved_path")?;
    let path = resolved.get("path").and_then(Value::as_str)?;
    let args = resolved.get("args");
    Some(format!("{}{}", path, format_type_args(args, state)))
}

fn borrowed_ref_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let borrowed = value.get("borrowed_ref")?;
    let is_mut = borrowed
        .get("is_mutable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let inner = borrowed.get("type").and_then(|inner| type_to_string(inner, state))?;
    Some(if is_mut {
        format!("&mut {inner}")
    } else {
        format!("&{inner}")
    })
}

fn raw_pointer_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let raw = value.get("raw_pointer")?;
    let is_mut = raw
        .get("is_mutable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let inner = raw.get("type").and_then(|inner| type_to_string(inner, state))?;
    Some(if is_mut {
        format!("*mut {inner}")
    } else {
        format!("*const {inner}")
    })
}

fn tuple_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let tuple = value.get("tuple").and_then(Value::as_array)?;
    let parts = tuple
        .iter()
        .filter_map(|inner| type_to_string(inner, state))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("({parts})"))
}

fn slice_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let slice = value.get("slice")?;
    let inner = type_to_string(slice, state)?;
    Some(format!("[{inner}]"))
}

fn array_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let array = value.get("array")?;
    let inner = array.get("type").and_then(|inner| type_to_string(inner, state))?;
    let len = array.get("len").and_then(Value::as_str).unwrap_or("");
    if len.is_empty() {
        Some(format!("[{inner}]"))
    } else {
        Some(format!("[{inner}; {len}]"))
    }
}

fn impl_trait_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let impl_trait = value.get("impl_trait").and_then(Value::as_array)?;
    let bounds = impl_trait
        .iter()
        .filter_map(|bound| trait_bound_to_string(bound, state))
        .collect::<Vec<_>>()
        .join(" + ");
    if bounds.is_empty() {
        Some("impl".to_string())
    } else {
        Some(format!("impl {bounds}"))
    }
}

fn dyn_trait_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let dyn_trait = value.get("dyn_trait")?;
    let traits = dyn_trait
        .get("traits")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|bound| trait_bound_to_string(bound, state))
                .collect::<Vec<_>>()
                .join(" + ")
        })
        .unwrap_or_default();
    if traits.is_empty() {
        Some("dyn".to_string())
    } else {
        Some(format!("dyn {traits}"))
    }
}

fn qualified_path_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let qualified = value.get("qualified_path")?;
    let name = qualified.get("name").and_then(Value::as_str).unwrap_or("");
    let self_type = qualified
        .get("self_type")
        .and_then(|inner| type_to_string(inner, state))
        .unwrap_or_default();
    let trait_name = qualified
        .get("trait")
        .and_then(|inner| inner.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if !trait_name.is_empty() {
        Some(format!("<{self_type} as {trait_name}>::{name}"))
    } else if !self_type.is_empty() {
        Some(format!("{self_type}::{name}"))
    } else {
        None
    }
}

fn function_pointer_type(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let fn_pointer = value.get("function_pointer")?;
    let decl = fn_pointer.get("decl")?;
    let params = decl
        .get("inputs")
        .and_then(Value::as_array)
        .map(|inputs| {
            inputs
                .iter()
                .filter_map(|pair| pair.as_array())
                .filter_map(|pair| pair.get(1))
                .filter_map(|param| type_to_string(param, state))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let output = decl
        .get("output")
        .and_then(|output| type_to_string(output, state))
        .unwrap_or_else(|| "()".to_string());
    Some(format!("fn({params}) -> {output}"))
}

fn format_type_args(args: Option<&Value>, state: &ParserState<'_>) -> String {
    let Some(args) = args else {
        return String::new();
    };
    let Some(angle) = args.get("angle_bracketed") else {
        return String::new();
    };
    let Some(items) = angle.get("args").and_then(Value::as_array) else {
        return String::new();
    };
    let mut rendered = Vec::new();
    for item in items {
        if let Some(ty) = item.get("type").and_then(|inner| type_to_string(inner, state)) {
            rendered.push(ty);
        } else if let Some(lifetime) = item.get("lifetime").and_then(Value::as_str) {
            rendered.push(lifetime.to_string());
        } else if let Some(const_val) = item.get("const").and_then(Value::as_str) {
            rendered.push(const_val.to_string());
        }
    }
    if rendered.is_empty() {
        String::new()
    } else {
        format!("<{}>", rendered.join(", "))
    }
}

fn trait_bound_to_string(value: &Value, state: &ParserState<'_>) -> Option<String> {
    let trait_bound = value.get("trait_bound")?;
    let trait_path = trait_bound.get("trait")?;
    let path = trait_path.get("path").and_then(Value::as_str)?;
    let args = trait_path.get("args");
    Some(format!("{}{}", path, format_type_args(args, state)))
}
fn parse_markdown_docs(raw: &str) -> ParsedDocs {
    let normalized = raw.replace("\r\n", "\n");
    let (preamble, sections) = split_sections(&normalized);
    let (summary, remarks) = split_summary_remarks(&preamble);

    let mut parsed = ParsedDocs {
        summary,
        remarks,
        returns: None,
        value: None,
        errors: None,
        panics: None,
        safety: None,
        deprecated: None,
        params: Vec::new(),
        type_params: Vec::new(),
        examples: Vec::new(),
        notes: Vec::new(),
        warnings: Vec::new(),
        see_also: Vec::new(),
        sections: Vec::new(),
    };

    for (title, body) in sections {
        let normalized_title = title.trim().to_ascii_lowercase();
        let trimmed_body = body.trim();
        if trimmed_body.is_empty() {
            continue;
        }
        match normalized_title.as_str() {
            "errors" => parsed.errors = Some(trimmed_body.to_string()),
            "panics" => parsed.panics = Some(trimmed_body.to_string()),
            "safety" => parsed.safety = Some(trimmed_body.to_string()),
            "returns" => parsed.returns = Some(trimmed_body.to_string()),
            "value" => parsed.value = Some(trimmed_body.to_string()),
            "deprecated" => parsed.deprecated = Some(trimmed_body.to_string()),
            "examples" | "example" => parsed.examples = extract_examples(trimmed_body),
            "notes" | "note" => parsed.notes.push(trimmed_body.to_string()),
            "warnings" | "warning" => parsed.warnings.push(trimmed_body.to_string()),
            "see also" | "seealso" | "see-also" => {
                parsed.see_also = parse_see_also_section(trimmed_body);
            }
            "arguments" | "args" | "parameters" | "params" => {
                parsed.params = parse_param_section(trimmed_body);
            }
            "type parameters" | "type params" | "typeparam" | "typeparams" => {
                parsed.type_params = parse_type_param_section(trimmed_body);
            }
            _ => parsed.sections.push(DocSection {
                title,
                body: trimmed_body.to_string(),
            }),
        }
    }

    parsed
}

fn parse_see_also_section(body: &str) -> Vec<SeeAlso> {
    let mut entries = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let item = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .unwrap_or(trimmed);
        if let Some(see) = parse_see_also_line(item) {
            entries.push(see);
        }
    }
    if entries.is_empty()
        && let Some(see) = parse_see_also_line(body.trim())
    {
        entries.push(see);
    }
    entries
}

fn parse_see_also_line(text: &str) -> Option<SeeAlso> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some((label, target)) = parse_markdown_link(trimmed) {
        return Some(SeeAlso {
            label: Some(label),
            target,
            target_kind: Some("markdown".to_string()),
        });
    }
    Some(SeeAlso {
        label: None,
        target: trimmed.to_string(),
        target_kind: Some("text".to_string()),
    })
}

fn parse_markdown_link(text: &str) -> Option<(String, String)> {
    let start = text.find('[')?;
    let remainder = &text[start + 1..];
    let mid = remainder.find("](")?;
    let label = remainder[..mid].trim();
    let tail = &remainder[mid + 2..];
    let end = tail.find(')')?;
    let target = tail[..end].trim();
    if label.is_empty() || target.is_empty() {
        return None;
    }
    Some((label.to_string(), target.to_string()))
}

fn split_sections(doc: &str) -> (String, Vec<(String, String)>) {
    let mut preamble = Vec::new();
    let mut sections = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_body = Vec::new();
    let mut in_code = false;

    for line in doc.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            if current_title.is_some() {
                current_body.push(line.to_string());
            } else {
                preamble.push(line.to_string());
            }
            continue;
        }
        if !in_code && let Some(title) = parse_heading(trimmed) {
            if let Some(active) = current_title.take() {
                sections.push((active, current_body.join("\n").trim().to_string()));
                current_body.clear();
            }
            current_title = Some(title);
            continue;
        }
        if current_title.is_some() {
            current_body.push(line.to_string());
        } else {
            preamble.push(line.to_string());
        }
    }

    if let Some(active) = current_title.take() {
        sections.push((active, current_body.join("\n").trim().to_string()));
    }

    (preamble.join("\n").trim().to_string(), sections)
}

fn parse_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    let hash_count = trimmed.chars().take_while(|ch| *ch == '#').count();
    if hash_count == 0 {
        return None;
    }
    let rest = trimmed[hash_count..].trim_start();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn split_summary_remarks(preamble: &str) -> (Option<String>, Option<String>) {
    let mut paragraphs = preamble
        .split("\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty());
    let summary = paragraphs.next().map(str::to_string);
    let rest = paragraphs.collect::<Vec<_>>().join("\n\n");
    let remarks = if rest.is_empty() {
        None
    } else {
        Some(rest)
    };
    (summary, remarks)
}

fn extract_examples(body: &str) -> Vec<DocExample> {
    let mut examples = Vec::new();
    let mut in_code = false;
    let mut current_lang: Option<String> = None;
    let mut current_code = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_code {
                let code = current_code.join("\n");
                if !code.trim().is_empty() {
                    examples.push(DocExample {
                        lang: current_lang.take(),
                        code: Some(code),
                        caption: None,
                    });
                }
                current_code.clear();
                in_code = false;
            } else {
                let lang = trimmed.trim_start_matches("```").trim();
                current_lang = if lang.is_empty() {
                    None
                } else {
                    Some(lang.to_string())
                };
                in_code = true;
            }
            continue;
        }
        if in_code {
            current_code.push(line.to_string());
        }
    }

    if !examples.is_empty() {
        return examples;
    }
    let trimmed = body.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![DocExample {
            lang: None,
            code: Some(trimmed.to_string()),
            caption: None,
        }]
    }
}

fn parse_param_section(body: &str) -> Vec<DocParam> {
    let mut params = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if !(trimmed.starts_with('-') || trimmed.starts_with('*')) {
            continue;
        }
        let item = trimmed.trim_start_matches(['-', '*']).trim();
        if item.is_empty() {
            continue;
        }
        if let Some((name, description)) = split_param_item(item) {
            params.push(DocParam {
                name,
                description,
                type_ref: None,
            });
        }
    }
    params
}

fn parse_type_param_section(body: &str) -> Vec<DocTypeParam> {
    let mut params = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if !(trimmed.starts_with('-') || trimmed.starts_with('*')) {
            continue;
        }
        let item = trimmed.trim_start_matches(['-', '*']).trim();
        if item.is_empty() {
            continue;
        }
        if let Some((name, description)) = split_param_item(item) {
            params.push(DocTypeParam { name, description });
        }
    }
    params
}

fn split_param_item(item: &str) -> Option<(String, Option<String>)> {
    let (name, description) = if let Some((name, rest)) = item.split_once(':') {
        (name, Some(rest))
    } else if let Some((name, rest)) = item.split_once(" - ") {
        (name, Some(rest))
    } else {
        (item, None)
    };

    let name = name.trim().trim_matches('`');
    if name.is_empty() {
        return None;
    }
    let description = description.map(|rest| rest.trim().to_string()).filter(|s| !s.is_empty());
    Some((name.to_string(), description))
}

#[cfg(test)]
mod tests {
    use super::parse_markdown_docs;

    #[test]
    fn parse_markdown_docs_extracts_see_also() {
        let docs = "Summary.\n\n# See Also\n- [Foo](crate::Foo)\n- Bar";
        let parsed = parse_markdown_docs(docs);

        assert_eq!(parsed.see_also.len(), 2);
        assert_eq!(parsed.see_also[0].label.as_deref(), Some("Foo"));
        assert_eq!(parsed.see_also[0].target, "crate::Foo");
        assert_eq!(parsed.see_also[1].label.as_deref(), None);
        assert_eq!(parsed.see_also[1].target, "Bar");
    }
}
