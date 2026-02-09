use rmcp::{
    ErrorData,
    model::{CallToolResult, Content},
    schemars,
    tool,
    tool_router,
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
                "help - List MCP commands to get context with how this MCP server works."
                    .to_string(),
                "ingestion_help - Details how to send code documentation to the MCP server for ingestion."
                    .to_string(),
                "ingest_csharp_xml - Ingest .NET XML documentation into the solution store."
                    .to_string(),
                "ingest_rustdoc_json - Ingest rustdoc JSON output into the solution store."
                    .to_string(),
                "list_ingests - List ingest metadata for a project."
                    .to_string(),
                "get_ingest - Fetch a specific ingest record by id."
                    .to_string(),
                "list_doc_sources - List document source metadata for a project."
                    .to_string(),
                "get_doc_source - Fetch a specific document source by id."
                    .to_string(),
                "get_symbol_adjacency - Fetch a symbol along with relation edges and related symbols."
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
    #[tool(description = "List the MCP commands to get context with how this MCP server works.")]
    async fn help(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::json(HelpCommands::default())?]))
    }

    #[tool(description = "Details how to send code documentation to the MCP server for ingestion")]
    async fn ingestion_help(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(
r"
1. Use the MCP ingestion tools to send documentation payloads into the server.
2. Required fields for all ingest tools:
    - solution: the solution/tenant name managed by the MCP server.
    - project_id: the project or crate identifier inside the solution.
    - xml/json: the documentation payload itself.
3. Optional metadata fields:
    - ingest_id: a caller-provided identifier to tag this ingest batch.
    - source_path: where the source documentation was generated (e.g. target/doc/<crate>.json).
    - source_modified_at: ISO-8601 timestamp for the source file.
    - tool_version: the tool version that produced the docs.
    - source_hash: a hash of the source documentation file.
4. Tool choices:
    - ingest_csharp_xml: use for .NET XML documentation payloads.
    - ingest_rustdoc_json: use for rustdoc JSON payloads emitted by `cargo doc`.
5. After ingestion, use the metadata and data tools to query projects, symbols, and doc blocks.
"
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
2.  The xml files must then be sent to the MCP server for ingestion, details in the mcp command `ingestion_help`.
3.  During ingestion, the symbols are stripped to a cannonical dataset form and a graph database is populated or updated.
4.  From the graph database, the other mcp commands can query for information about the code and relationships.
"
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
3.  The JSON files are sent to the MCP server for ingestion, details in the mcp command `ingestion_help`.
4.  During ingestion, symbols are stripped to a cannonical dataset form and a graph database is populated or updated.
    From the graph database, the other mcp commands can query for information about the code and relationships.
"#
        )]))
    }
}
