use rmcp::{
    ErrorData,
    model::{CallToolResult, Content},
    schemars, tool, tool_router,
};
use serde::{Deserialize, Serialize};
use surrealdb::Connection;

use crate::DocxMcp;

/// Payload listing context-focused MCP commands.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HelpCommands {
    pub commands: Vec<String>,
}

impl Default for HelpCommands {
    fn default() -> Self {
        Self {
            commands: vec![
                "skills - Comprehensive agent guide (skills.md). Call this tool to retrieve the full text; save the output as `skills.md` in your project root for offline reference."
                    .to_string(),
                "help - List MCP commands to get context with how this MCP server works."
                    .to_string(),
                "version - Get the MCP server version."
                    .to_string(),
                "ingestion_help - Details how to send code documentation to the MCP server for ingestion."
                    .to_string(),
                "ingest_csharp_xml - Ingest .NET XML documentation into the solution store (xml or xml_path)."
                    .to_string(),
                "ingest_rustdoc_json - Ingest rustdoc JSON output into the solution store (json or json_path)."
                    .to_string(),
                "list_projects - List projects for a solution."
                    .to_string(),
                "search_projects - Search projects by wildcard pattern (e.g. docx*)."
                    .to_string(),
                "list_ingests - List ingest metadata for a project."
                    .to_string(),
                "get_ingest - Fetch a specific ingest record by id."
                    .to_string(),
                "delete_solution - Delete an entire solution database (destructive; requires confirm=true)."
                    .to_string(),
                "list_doc_sources - List document source metadata for a project."
                    .to_string(),
                "get_doc_source - Fetch a specific document source by id."
                    .to_string(),
                "list_symbol_types - List symbol kinds present in a project."
                    .to_string(),
                "search_symbols - Search symbols by name fragment."
                    .to_string(),
                "search_symbols_advanced - Search symbols by optional filters (name, qualified_name, symbol_key, signature)."
                    .to_string(),
                "get_symbol - Fetch a symbol by its key."
                    .to_string(),
                "list_doc_blocks - List doc blocks for a symbol."
                    .to_string(),
                "search_doc_blocks - Search doc blocks by text fragment."
                    .to_string(),
                "get_symbol_adjacency - Fetch a symbol along with relation edges and related symbols."
                    .to_string(),
                "audit_project_completeness - Report per-project counts for symbols/docs/relations and missing source metadata."
                    .to_string(),
                "dotnet_help - Describes how .net solutions are processed and ingested."
                    .to_string(),
                "rust_help - Describes how rust solutions are processed and ingested."
                    .to_string()
            ],
        }
    }
}

#[tool_router(router = tool_router_context, vis = "pub")]
impl<C: Connection> DocxMcp<C> {
    #[tool(
        description = "Returns the full skills.md agent guide: when and how to use each tool, common workflows, decision trees, and troubleshooting. Save the output as `skills.md` in your project root for offline reference; if the file already exists locally, read it instead of calling this tool again."
    )]
    async fn skills(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(include_str!(
            "../../skills.md"
        ))]))
    }

    #[tool(description = "List the MCP commands to get context with how this MCP server works.")]
    async fn help(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::json(
            HelpCommands::default(),
        )?]))
    }

    #[tool(description = "Details how to send code documentation to the MCP server for ingestion")]
    async fn ingestion_help(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(
            r#"
1. Use the MCP ingestion tools to send documentation payloads into the server.
2. Required fields for all ingest tools:
    - solution: the solution/tenant name managed by the MCP server.
    - project_id: the project or crate identifier inside the solution.
    - documentation payload: provide raw XML/JSON file contents (full text), or use *_path.
3. Optional metadata fields:
    - ingest_id: a caller-provided identifier to tag this ingest batch.
    - source_path: where the source documentation was generated (e.g. target/doc/<crate>.json).
    - source_modified_at: ISO-8601 timestamp for the source file.
    - tool_version: the tool version that produced the docs.
    - source_hash: a hash of the source documentation file.
4. Tool choices:
    - ingest_csharp_xml: use for raw .NET XML documentation payloads (xml or xml_path).
    - ingest_rustdoc_json: use for raw rustdoc JSON payloads (json or json_path).
5. Payload options (MCP tools and HTTP ingest):
    - Provide exactly one of:
        - xml/json: raw file contents (full text). For rustdoc, json must be the full rustdoc JSON document.
        - xml_path/json_path: path to a file on the MCP server host.
      Empty strings are treated as missing.
6. HTTP ingest endpoint (when MCP tool payloads are too large):
    - POST to /ingest with JSON payload:
      {
        "solution": "<solution>",
        "project_id": "<project_id>",
        "kind": "csharp_xml" | "rustdoc_json",
        "contents": "<raw file contents>",
        "contents_path": "<optional server path>",
        "ingest_id": "<optional>",
        "source_path": "<optional>",
        "source_modified_at": "<optional>",
        "tool_version": "<optional>",
        "source_hash": "<optional>"
      }
    - Required for HTTP ingest: solution, project_id, kind, and either contents or contents_path.
    - contents_path must be readable from the server host. If the server runs in Docker,
      mount the file into the container (e.g. -v <host_dir>:/data) and send /data/<file>.
7. If the AI cannot send the full file content in one MCP tool call:
    - Use a terminal command to POST the file directly (avoids pasting the full payload).
      Example with curl (Linux/macOS, C# XML raw contents):
        python3 - <<'PY' > payload.json
        import json, pathlib
        print(json.dumps({
          "solution": "my-solution",
          "project_id": "MyAssembly",
          "kind": "csharp_xml",
          "contents": pathlib.Path("/path/MyAssembly.xml").read_text()
        }))
        PY
        curl -X POST http://127.0.0.1:4010/ingest \
          -H "Content-Type: application/json" \
          --data-binary @payload.json
      Example with curl (Linux/macOS, rustdoc JSON raw contents):
        python3 - <<'PY' > payload.json
        import json, pathlib
        print(json.dumps({
          "solution": "docx",
          "project_id": "docx-core",
          "kind": "rustdoc_json",
          "contents": pathlib.Path("target/doc/docx_core.json").read_text()
        }))
        PY
        curl -X POST http://127.0.0.1:4010/ingest \
          -H "Content-Type: application/json" \
          --data-binary @payload.json
      Example with PowerShell (C# XML raw contents):
        $xml = Get-Content -Raw "C:\path\MyAssembly.xml"
        $body = @{ solution = "my-solution"; project_id = "MyAssembly"; kind = "csharp_xml"; contents = $xml } | ConvertTo-Json
        Invoke-WebRequest -Uri http://127.0.0.1:4010/ingest -Method Post -ContentType "application/json" -Body $body
      Example with PowerShell (rustdoc JSON raw contents):
        $body = @{ solution = "docx"; project_id = "docx-core"; kind = "rustdoc_json"; contents = Get-Content -Raw "target\doc\docx_core.json" } | ConvertTo-Json
        Invoke-WebRequest -Uri http://127.0.0.1:4010/ingest -Method Post -ContentType "application/json" -Body $body
      Example with PowerShell (file path on server):
        $body = @{ solution = "docx"; project_id = "docx-core"; kind = "rustdoc_json"; contents_path = "target\doc\docx_core.json" } | ConvertTo-Json
        Invoke-WebRequest -Uri http://127.0.0.1:4010/ingest -Method Post -ContentType "application/json" -Body $body
    - If the payload exceeds the ingest size limit, increase DOCX_INGEST_MAX_BODY_BYTES or emit a smaller doc set.
8. After ingestion, use the metadata and data tools to query projects, symbols, and doc blocks.
9. Tip: call the `skills` tool to retrieve the full skills.md guide covering workflows, decision trees, and troubleshooting.
   Save the output as `skills.md` in your project root for offline reference.
 "#,
        )]))
    }

    #[tool(description = "Describes how .net solutions are processed and ingested.")]
    async fn dotnet_help(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(
            r"
1.  .net doc comments in XML format must be emitted from .net solution projects.
    To do this, you have two ways:
        1.  Add a `Directory.Build.props` file to the solution root with the following content:
            ```
            <Project>
                <PropertyGroup>
                    <GenerateDocumentationFile>true</GenerateDocumentationFile>
                </PropertyGroup>
            </Project>
            ```
        2.  The project file (e.g. .csproj) can be modified to include the following:
            ```
            <PropertyGroup>
                <GenerateDocumentationFile>true</GenerateDocumentationFile>
            </PropertyGroup>
            ```
    You can also similarly disable a project from emitting .net xml documentation by setting the following in a project file (e.g. .csproj):
        ```
        <PropertyGroup>
                <GenerateDocumentationFile>false</GenerateDocumentationFile>
        </PropertyGroup>
        ```
    .net doc XML is emitted beside the assembly in the `bin/<configuration>/<netstandard>/` folder.
    e.g. `bin\Debug\net10.0` or `bin\Release\net9.0\`
2.  The xml files must then be sent to the MCP server for ingestion (kind=csharp_xml); see `ingestion_help`.
    If using contents_path, the file path must be readable from the server host. When running in Docker,
    mount the file into the container or send raw contents instead.
3.  During ingestion, the symbols are stripped to a cannonical dataset form and a graph database is populated or updated.
4.  From the graph database, the other mcp commands can query for information about the code and relationships.

Note: for a comprehensive workflow guide, call the `skills` tool. It returns the full skills.md agent guide
covering decision trees, common patterns, and troubleshooting. Save it as `skills.md` in your project root
for offline reference.
",
        )]))
    }

    #[tool(description = "Describes how rust workspaces or crates are processed and ingested.")]
    async fn rust_help(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(
            r#"
1.  JSON rustdoc must be emitted from the Rust workspace using nightly.
    Nightly is required because rustdoc JSON is currently unstable.
2.  Example commands (choose one):
        1.  Per-project: `cargo +nightly rustdoc -p <project> --<crate-type> -Z unstable-options --output-format json --document-private-items`
        2.  Per-workspace: Windows PowerShell:
            `$env:RUSTDOCFLAGS = "-Z unstable-options --output-format json"; cargo doc --workspace --no-deps --document-private-items`
        3.  Per-workspace: Unix shell:
            `RUSTDOCFLAGS="-Z unstable-options --output-format json" cargo doc --workspace --no-deps --document-private-items`
    You can also do exclusions. Examples:
        1.  `$env:RUSTDOCFLAGS = "-Z unstable-options --output-format json"; cargo doc --workspace --exclude <crate_name> --no-deps --document-private-items`
        2.  `RUSTDOCFLAGS="-Z unstable-options --output-format json" cargo doc --workspace --exclude <crate_name> --exclude <crate_name> --no-deps --document-private-items`
    It may be beneficial to set up a build.rs to automate the generation of rustdoc JSON.
    All rustdoc emission is in <root>/target/doc
3.  The JSON files are sent to the MCP server for ingestion (kind=rustdoc_json); see `ingestion_help`.
    If using contents_path, the file path must be readable from the server host. When running in Docker,
    mount the file into the container or send raw contents instead.
4.  During ingestion, symbols are stripped to a cannonical dataset form and a graph database is populated or updated.
    From the graph database, the other mcp commands can query for information about the code and relationships.

Note: for a comprehensive workflow guide, call the `skills` tool. It returns the full skills.md agent guide
covering decision trees, common patterns, and troubleshooting. Save it as `skills.md` in your project root
for offline reference.
"#,
        )]))
    }
}
