use std::{error::Error, fmt, path::Path};

use docx_store::models::{
    DocBlock,
    DocExample,
    DocException,
    DocInherit,
    DocParam,
    DocTypeParam,
    SeeAlso,
    SourceId,
    Symbol,
};
use docx_store::schema::{SOURCE_KIND_CSHARP_XML, make_csharp_symbol_key};
use roxmltree::{Document, Node};

/// Options for parsing C# XML documentation.
#[derive(Debug, Clone)]
pub struct CsharpParseOptions {
    pub project_id: String,
    pub ingest_id: Option<String>,
    pub language: String,
    pub source_kind: String,
}

impl CsharpParseOptions {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            ingest_id: None,
            language: "csharp".to_string(),
            source_kind: SOURCE_KIND_CSHARP_XML.to_string(),
        }
    }

    #[must_use]
    pub fn with_ingest_id(mut self, ingest_id: impl Into<String>) -> Self {
        self.ingest_id = Some(ingest_id.into());
        self
    }
}

/// Output from parsing C# XML documentation.
#[derive(Debug, Clone)]
pub struct CsharpParseOutput {
    pub assembly_name: Option<String>,
    pub symbols: Vec<Symbol>,
    pub doc_blocks: Vec<DocBlock>,
}

/// Error type for C# XML parse failures.
#[derive(Debug)]
pub struct CsharpParseError {
    message: String,
}

impl CsharpParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CsharpParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "C# XML parse error: {}", self.message)
    }
}

impl Error for CsharpParseError {}

impl From<roxmltree::Error> for CsharpParseError {
    fn from(err: roxmltree::Error) -> Self {
        Self::new(err.to_string())
    }
}

impl From<std::io::Error> for CsharpParseError {
    fn from(err: std::io::Error) -> Self {
        Self::new(err.to_string())
    }
}

impl From<tokio::task::JoinError> for CsharpParseError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::new(err.to_string())
    }
}

/// Parser for C# XML documentation files.
pub struct CsharpXmlParser;

impl CsharpXmlParser {
    /// Parses C# XML documentation into symbols and doc blocks.
    ///
    /// # Errors
    /// Returns `CsharpParseError` if the XML is invalid or cannot be parsed.
    #[allow(clippy::too_many_lines)]
    pub fn parse(xml: &str, options: &CsharpParseOptions) -> Result<CsharpParseOutput, CsharpParseError> {
        let doc = Document::parse(xml)?;
        let assembly_name = extract_assembly_name(&doc);
        let mut symbols = Vec::new();
        let mut doc_blocks = Vec::new();

        for member in doc.descendants().filter(|node| node.has_tag_name("member")) {
            let Some(doc_id) = member.attribute("name") else {
                continue;
            };

            let symbol_key = make_csharp_symbol_key(&options.project_id, doc_id);
            let parts = parse_doc_id(doc_id);

            let mut symbol = Symbol {
                id: None,
                project_id: options.project_id.clone(),
                language: Some(options.language.clone()),
                symbol_key: symbol_key.clone(),
                kind: parts.kind,
                name: parts.name,
                qualified_name: parts.qualified_name,
                display_name: parts.display_name,
                signature: parts.signature,
                signature_hash: None,
                visibility: None,
                is_static: None,
                is_async: None,
                is_const: None,
                is_deprecated: None,
                since: None,
                stability: None,
                source_path: None,
                line: None,
                col: None,
                return_type: None,
                params: Vec::new(),
                type_params: Vec::new(),
                attributes: Vec::new(),
                source_ids: vec![SourceId {
                    kind: "csharp_doc_id".to_string(),
                    value: doc_id.to_string(),
                }],
                doc_summary: None,
                extra: None,
            };

            let mut doc_block = DocBlock {
                id: None,
                project_id: options.project_id.clone(),
                ingest_id: options.ingest_id.clone(),
                symbol_key: Some(symbol_key.clone()),
                language: Some(options.language.clone()),
                source_kind: Some(options.source_kind.clone()),
                doc_hash: None,
                summary: None,
                remarks: None,
                returns: None,
                value: None,
                params: Vec::new(),
                type_params: Vec::new(),
                exceptions: Vec::new(),
                examples: Vec::new(),
                notes: Vec::new(),
                warnings: Vec::new(),
                safety: None,
                panics: None,
                errors: None,
                see_also: Vec::new(),
                deprecated: None,
                inherit_doc: None,
                sections: Vec::new(),
                raw: None,
                extra: None,
            };

            for child in member.children().filter(Node::is_element) {
                match child.tag_name().name() {
                    "summary" => doc_block.summary = optional_text(child),
                    "remarks" => doc_block.remarks = optional_text(child),
                    "returns" => doc_block.returns = optional_text(child),
                    "value" => doc_block.value = optional_text(child),
                    "param" => {
                        if let Some(name) = child.attribute("name") {
                        let description = render_doc_text(child);
                        doc_block.params.push(DocParam {
                            name: name.to_string(),
                            description: if description.is_empty() { None } else { Some(description) },
                            type_ref: None,
                        });
                        }
                    }
                    "typeparam" => {
                        if let Some(name) = child.attribute("name") {
                        let description = render_doc_text(child);
                        doc_block.type_params.push(DocTypeParam {
                            name: name.to_string(),
                            description: if description.is_empty() { None } else { Some(description) },
                        });
                        }
                    }
                    "exception" => {
                        let description = render_doc_text(child);
                        let type_ref = child
                            .attribute("cref")
                            .map(|cref| docx_store::models::TypeRef {
                                display: Some(cref.to_string()),
                                canonical: Some(cref.to_string()),
                                language: Some(options.language.clone()),
                                symbol_key: Some(make_csharp_symbol_key(&options.project_id, cref)),
                                generics: Vec::new(),
                                modifiers: Vec::new(),
                            });
                        doc_block.exceptions.push(DocException {
                            type_ref,
                            description: if description.is_empty() { None } else { Some(description) },
                        });
                    }
                    "example" => {
                        let text = render_doc_text(child);
                        if !text.is_empty() {
                            doc_block.examples.push(DocExample {
                                lang: None,
                                code: Some(text),
                                caption: None,
                            });
                        }
                    }
                    "seealso" => {
                        if let Some(see) = parse_see_also(child) {
                            doc_block.see_also.push(see);
                        }
                    }
                    "note" => {
                        let text = render_doc_text(child);
                        if !text.is_empty() {
                            doc_block.notes.push(text);
                        }
                    }
                    "warning" => {
                        let text = render_doc_text(child);
                        if !text.is_empty() {
                            doc_block.warnings.push(text);
                        }
                    }
                    "inheritdoc" => {
                        let cref = child.attribute("cref").map(str::to_string);
                        let path = child.attribute("path").map(str::to_string);
                        doc_block.inherit_doc = Some(DocInherit { cref, path });
                    }
                    "deprecated" => {
                        let text = render_doc_text(child);
                        if !text.is_empty() {
                            doc_block.deprecated = Some(text);
                        }
                    }
                    _ => {}
                }
            }

            if doc_block.summary.is_some() {
                symbol.doc_summary.clone_from(&doc_block.summary);
            }

            let range = member.range();
            doc_block.raw = Some(xml[range].to_string());

            symbols.push(symbol);
            doc_blocks.push(doc_block);
        }

        Ok(CsharpParseOutput {
            assembly_name,
            symbols,
            doc_blocks,
        })
    }

    /// Parses XML asynchronously using a blocking task.
    ///
    /// # Errors
    /// Returns `CsharpParseError` if parsing fails or the task panics.
    pub async fn parse_async(
        xml: String,
        options: CsharpParseOptions,
    ) -> Result<CsharpParseOutput, CsharpParseError> {
        tokio::task::spawn_blocking(move || Self::parse(&xml, &options)).await?
    }

    /// Parses XML from a file path asynchronously.
    ///
    /// # Errors
    /// Returns `CsharpParseError` if the file cannot be read or the XML cannot be parsed.
    pub async fn parse_file(
        path: impl AsRef<Path>,
        options: CsharpParseOptions,
    ) -> Result<CsharpParseOutput, CsharpParseError> {
        let path = path.as_ref().to_path_buf();
        let xml = tokio::task::spawn_blocking(move || std::fs::read_to_string(path)).await??;
        Self::parse_async(xml, options).await
    }
}

#[derive(Debug)]
struct DocIdParts {
    kind: Option<String>,
    name: Option<String>,
    qualified_name: Option<String>,
    display_name: Option<String>,
    signature: Option<String>,
}

fn parse_doc_id(doc_id: &str) -> DocIdParts {
    let mut parts = doc_id.splitn(2, ':');
    let prefix = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");

    let kind = match prefix {
        "T" => Some("type".to_string()),
        "M" => Some("method".to_string()),
        "P" => Some("property".to_string()),
        "F" => Some("field".to_string()),
        "E" => Some("event".to_string()),
        "N" => Some("namespace".to_string()),
        _ => None,
    };

    let (qualified_name, signature) = if rest.is_empty() {
        (None, None)
    } else if let Some(pos) = rest.find('(') {
        let qualified = rest[..pos].to_string();
        (Some(qualified), Some(rest.to_string()))
    } else {
        (Some(rest.to_string()), Some(rest.to_string()))
    };

    let name = qualified_name
        .as_deref()
        .and_then(extract_simple_name)
        .map(str::to_string);

    DocIdParts {
        kind,
        name: name.clone(),
        qualified_name,
        display_name: name,
        signature,
    }
}

fn extract_simple_name(value: &str) -> Option<&str> {
    value.rsplit(['.', '+', '#']).next()
}

fn extract_assembly_name(doc: &Document<'_>) -> Option<String> {
    let assembly_node = doc.descendants().find(|node| node.has_tag_name("assembly"))?;
    let name_node = assembly_node
        .children()
        .find(|node| node.has_tag_name("name"))?;
    name_node.text().map(|text| text.trim().to_string())
}

fn render_doc_text(node: Node<'_, '_>) -> String {
    let text = render_children(node);
    cleanup_text(&text)
}

fn optional_text(node: Node<'_, '_>) -> Option<String> {
    let text = render_doc_text(node);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn render_children(node: Node<'_, '_>) -> String {
    let mut output = String::new();
    for child in node.children() {
        let fragment = render_node(child);
        if fragment.is_empty() {
            continue;
        }
        if needs_space(&output, &fragment) {
            output.push(' ');
        }
        output.push_str(&fragment);
    }
    output
}

fn render_node(node: Node<'_, '_>) -> String {
    match node.node_type() {
        roxmltree::NodeType::Text => node.text().unwrap_or("").to_string(),
        roxmltree::NodeType::Element => match node.tag_name().name() {
            "para" => {
                let text = render_children(node);
                if text.is_empty() {
                    String::new()
                } else {
                    format!("\n{}\n", text.trim())
                }
            }
            "code" => render_code_block(node),
            "see" | "seealso" => render_inline_link(node),
            "paramref" | "typeparamref" => render_ref(node),
            "list" => render_list(node),
            _ => render_children(node),
        },
        _ => String::new(),
    }
}

fn render_code_block(node: Node<'_, '_>) -> String {
    let code_text = node.text().unwrap_or("").trim();
    if code_text.is_empty() {
        String::new()
    } else {
        format!("\n```\n{code_text}\n```\n")
    }
}

fn render_inline_link(node: Node<'_, '_>) -> String {
    let target = node
        .attribute("cref")
        .or_else(|| node.attribute("href"))
        .unwrap_or("")
        .trim();
    let label = node.text().unwrap_or("").trim();
    if target.is_empty() {
        label.to_string()
    } else if label.is_empty() {
        target.to_string()
    } else {
        format!("[{label}]({target})")
    }
}

fn render_ref(node: Node<'_, '_>) -> String {
    let name = node.attribute("name").unwrap_or("").trim();
    if name.is_empty() {
        String::new()
    } else {
        format!("`{name}`")
    }
}

fn render_list(node: Node<'_, '_>) -> String {
    let mut lines = Vec::new();
    for item in node.children().filter(|child| child.has_tag_name("item")) {
        let term = item
            .children()
            .find(|child| child.has_tag_name("term"))
            .map(render_children);
        let description = item
            .children()
            .find(|child| child.has_tag_name("description"))
            .map(render_children);
        let text = match (term, description) {
            (Some(term), Some(description)) => format!("{}: {}", term.trim(), description.trim()),
            (Some(term), None) => term,
            (None, Some(description)) => description,
            (None, None) => render_children(item),
        };
        let text = text.trim();
        if !text.is_empty() {
            lines.push(format!("- {text}"));
        }
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("\n{}\n", lines.join("\n"))
    }
}

fn cleanup_text(value: &str) -> String {
    let mut lines = Vec::new();
    let mut in_code_block = false;
    for line in value.replace("\r\n", "\n").lines() {
        let trimmed = line.trim_end();
        if trimmed.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            lines.push(trimmed.to_string());
            continue;
        }
        if in_code_block {
            lines.push(trimmed.to_string());
        } else {
            lines.push(collapse_whitespace(trimmed).trim().to_string());
        }
    }

    while matches!(lines.first(), Some(line) if line.is_empty()) {
        lines.remove(0);
    }
    while matches!(lines.last(), Some(line) if line.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

fn collapse_whitespace(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_space = false;
    for ch in value.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                output.push(' ');
                last_was_space = true;
            }
        } else {
            output.push(ch);
            last_was_space = false;
        }
    }
    output
}

fn needs_space(current: &str, next: &str) -> bool {
    if current.is_empty() {
        return false;
    }
    let current_last = current.chars().last();
    let next_first = next.chars().next();
    matches!(current_last, Some(ch) if !ch.is_whitespace() && ch != '\n')
        && matches!(next_first, Some(ch) if !ch.is_whitespace() && ch != '\n')
}

fn parse_see_also(node: Node<'_, '_>) -> Option<SeeAlso> {
    let target = node
        .attribute("cref")
        .or_else(|| node.attribute("href"))
        .map(str::to_string)?;
    let label = node.text().map(|text| text.trim().to_string());
    let label = match label {
        Some(text) if text.is_empty() => None,
        other => other,
    };
    let target_kind = if node.attribute("cref").is_some() {
        Some("cref".to_string())
    } else {
        Some("href".to_string())
    };
    Some(SeeAlso {
        label,
        target,
        target_kind,
    })
}
