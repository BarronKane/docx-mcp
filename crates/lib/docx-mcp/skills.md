---
name: docx-mcp
description: Guide for using the docx-mcp MCP server to ingest and query code documentation graphs.
metadata:
  short-description: Ingest and explore code docs with docx-mcp
---

# docx-mcp Skills

You are connected to **docx-mcp**, an MCP server that ingests code documentation (C# XML docs, Rust rustdoc JSON) into a normalized graph database and exposes it for structured querying. It turns raw documentation artifacts into a searchable, relationship-aware knowledge graph.

## When to Use This Server

Use docx-mcp tools when you need to:
- **Understand a codebase's public API** without reading every source file.
- **Look up documentation** for a specific type, method, function, module, or namespace.
- **Explore relationships** between symbols (inheritance, containment, type references, see-also links).
- **Search across documentation** for concepts, error descriptions, or parameter names.
- **Verify signatures and return types** for functions and methods you're about to call or modify.
- **Discover what symbols exist** in a project before diving into source code.

Do **not** use docx-mcp when:
- You need to read or edit source code directly (use file tools for that).
- The project has no ingested documentation yet (ingest first, then query).
- You need runtime behavior or test results (this is static documentation only).

---

## Core Concepts

### Solution
A **solution** is the top-level tenant. It maps to a SurrealDB database. Use the name of the workspace, repository, or solution directory. If unsure, call `list_solutions` to see what exists, or choose a new name.

### Project
A **project** (`project_id`) is a crate, assembly, or library within a solution. For Rust, this is typically the crate name. For .NET, it's the assembly name.

### Ingest ID
`ingest_id` may be caller-provided during ingestion. Internally, ingest records are project-scoped (`<project_id>::<ingest_id>`), while doc sources usually store the requested value (`<ingest_id>`).
- `list_ingests` returns the scoped ingest id.
- `get_ingest` accepts the scoped ingest id directly.
- `get_ingest` also accepts the requested id only when it is unique across projects in the same solution.
- `list_doc_sources` ingest filters accept either form (`smoke` or `MyProject::smoke`).

### Symbol Key
Symbols are identified by a composite key: `{language}|{project_id}|{qualified_name}`.
- Rust example: `rust|docx_core|docx_core::control::DocxControlPlane`
- C# example: `csharp|MyAssembly|T:MyNamespace.MyClass`

### Relations
The graph stores typed edges between symbols:
| Relation | Meaning |
|---|---|
| `member_of` | Symbol is a member of a parent (method in a struct, field in a class) |
| `contains` | Inverse of member_of (parent contains child) |
| `returns` | Function/method returns this type |
| `param_type` | Function/method has a parameter of this type |
| `see_also` | Documentation cross-reference |
| `inherits` | Type inheritance relation |
| `references` | Documentation references this symbol (e.g. exception types) |
| `observed_in` | Symbol was observed in a specific ingested documentation source |

---

## Workflow

### Step 1: Check Existing State
```
list_solutions          -- What solutions exist?
list_projects           -- What projects are in this solution?
search_projects         -- Find projects by pattern (e.g. "docx*")
```

### Step 2: Ingest Documentation (if needed)

#### For Rust Crates
1. Generate rustdoc JSON (requires nightly):
   ```bash
   cargo +nightly rustdoc -p <crate> -Z unstable-options --output-format json --document-private-items
   ```
   Or for the whole workspace:
   ```bash
   RUSTDOCFLAGS="-Z unstable-options --output-format json" cargo doc --workspace --no-deps --document-private-items
   ```
2. Ingest via MCP tool:
   ```
   ingest_rustdoc_json(solution, project_id, json_path="target/doc/<crate_name>.json")
   ```
3. For large files, use the HTTP ingest endpoint instead:
   ```bash
   curl -X POST http://127.0.0.1:4010/ingest \
     -H "Content-Type: application/json" \
     -d '{"solution":"my-sol","project_id":"my-crate","kind":"rustdoc_json","contents_path":"target/doc/my_crate.json"}'
   ```

#### For .NET Projects
1. Enable XML doc generation in the project or `Directory.Build.props`:
   ```xml
   <PropertyGroup>
     <GenerateDocumentationFile>true</GenerateDocumentationFile>
   </PropertyGroup>
   ```
2. Build the project to generate XML files in `bin/<config>/<tfm>/`.
3. Ingest via MCP tool:
   ```
   ingest_csharp_xml(solution, project_id, xml_path="bin/Debug/net9.0/MyAssembly.xml")
   ```

#### Choosing Between MCP Tool and HTTP Ingest
- **MCP tool** (`ingest_rustdoc_json`, `ingest_csharp_xml`): Use for small-to-medium payloads. Pass `json`/`xml` for inline content or `json_path`/`xml_path` for server-local file paths.
- **HTTP ingest** (`POST /ingest`): Use when MCP tool payload limits are exceeded. Supports `contents` (raw text) or `contents_path` (server-accessible file path). Max body size default: 25MB (configurable via `DOCX_INGEST_MAX_BODY_BYTES`).

### Step 3: Explore the Graph

#### Discovery (broad to narrow)
```
list_symbol_types       -- What kinds of symbols exist? (struct, function, module, etc.)
get_members             -- List members under a namespace/module scope
search_symbols          -- Find symbols by name fragment
search_symbols_advanced -- Exact/fuzzy multi-filter symbol search
```

#### Detail Retrieval
```
get_symbol              -- Full symbol metadata (signature, params, return type, source location)
list_doc_blocks         -- Documentation blocks for a symbol (summary, remarks, examples, params)
get_symbol_adjacency    -- Symbol + all relations + related symbols (the richest single query)
```

#### Documentation Search
```
search_doc_blocks       -- Full-text search across doc summaries, remarks, and return descriptions
```

#### Metadata Inspection
```
list_ingests            -- Ingestion history for a project
get_ingest              -- Details of a specific ingest run
list_doc_sources        -- Source file metadata for ingested docs
get_doc_source          -- Details of a specific doc source
audit_project_completeness -- Coverage counts for symbols, docs, and relations
```

---

## Decision Guide: Which Tool to Use

| I want to... | Use this tool |
|---|---|
| See what's been ingested | `list_solutions` then `list_projects` |
| Find a type or function by name | `search_symbols` with a name fragment |
| Read the docs for a specific symbol | `list_doc_blocks` with the symbol_key |
| Understand a symbol's full context | `get_symbol_adjacency` (returns symbol + docs + relations) |
| Browse a namespace or module | `get_members` with the scope (qualified name prefix) |
| Find docs mentioning a concept | `search_doc_blocks` with a text fragment |
| Find a symbol with exact key/signature filters | `search_symbols_advanced` |
| Check what kinds of things a project has | `list_symbol_types` |
| Get a symbol's signature and parameters | `get_symbol` |
| See what a function returns or takes | `get_symbol_adjacency` (check `returns` and `param_types`) |
| Trace inheritance | `get_symbol_adjacency` (check `inherits`) |
| Check ingestion/completeness coverage quickly | `audit_project_completeness` |
| Verify the server is running | `health` |

---

## Common Patterns

### Pattern: Understand an Unfamiliar Crate
1. `list_projects(solution)` -- see what's ingested
2. `list_symbol_types(solution, project_id)` -- what's in here?
3. `get_members(solution, project_id, scope="<crate_name>")` -- top-level items
4. `search_symbols(solution, project_id, name="<keyword>")` -- find specific things
5. `get_symbol_adjacency(solution, project_id, symbol_key)` -- deep dive

### Pattern: Look Up a Function Before Calling It
1. `search_symbols(solution, project_id, name="function_name")`
2. `get_symbol(solution, project_id, symbol_key)` -- get signature, params, return type
3. `list_doc_blocks(solution, project_id, symbol_key)` -- read the docs, examples, errors

### Pattern: Explore Type Hierarchy
1. `get_symbol_adjacency(solution, project_id, symbol_key)` for the base type
2. Check `inherits` and `contains` edges in the result
3. Follow related symbol keys for connected types

### Pattern: Exact Symbol Lookup
1. `search_symbols_advanced(solution, project_id, symbol_key="...")`
2. If needed, add `qualified_name` or `signature` filters to disambiguate
3. `get_symbol_adjacency(solution, project_id, symbol_key)` for full context

### Pattern: Search for Error Handling Guidance
1. `search_doc_blocks(solution, project_id, text="error")` or `text="panic"`
2. Review the `errors`, `panics`, and `exceptions` fields in returned doc blocks

---

## Anti-Patterns

- **Don't pass both inline content and a file path** -- provide exactly one of `xml`/`json` or `xml_path`/`json_path`.
- **Don't guess symbol keys** -- use `search_symbols` to find the correct key first, then use it in subsequent queries.
- **Don't skip the solution parameter** -- every query tool requires `solution`. Use `list_solutions` if unsure.
- **Don't re-ingest unnecessarily** -- check `list_ingests` to see if documentation is already current.
- **Don't assume unscoped ingest ids are always resolvable** -- if the same requested `ingest_id` is reused across projects, use the scoped form (`project::ingest`) for `get_ingest`.
- **Don't use `get_symbol_adjacency` for simple lookups** -- if you only need the docs, `list_doc_blocks` is lighter. Use adjacency when you need the relationship graph.

---

## Troubleshooting

| Problem | Solution |
|---|---|
| "unknown solution" error | The solution hasn't been created yet. Ingest documentation first, or check the name with `list_solutions`. |
| Empty results from search | Documentation may not be ingested yet. Run `list_projects` to verify, then ingest if needed. |
| Ingest fails with payload too large | Use the HTTP ingest endpoint (`POST /ingest`) or `contents_path` instead of inline content. |
| `contents_path` not found | The path must be accessible from the server host. If using Docker, mount the file into the container. |
| Symbol key not found | Symbol keys are case-sensitive and language-prefixed. Use `search_symbols` to find the exact key. |
| `get_ingest` says id is ambiguous | Use the project-scoped id from `list_ingests` (format: `<project_id>::<requested_ingest_id>`). |
| `list_doc_sources` filtered by ingest id is empty | Try either ingest form: requested (`smoke`) or scoped (`MyProject::smoke`). |
| Rustdoc JSON generation fails | Requires Rust nightly. Use `cargo +nightly rustdoc` with `-Z unstable-options --output-format json`. |
| No XML generated for .NET project | Ensure `<GenerateDocumentationFile>true</GenerateDocumentationFile>` is set and rebuild. |

---

## Tool Reference (Quick)

### Lifecycle
| Tool | Purpose |
|---|---|
| `health` | Returns "ok" if server is running |
| `version` | Returns server name and version |
| `skills` | Returns this guide |
| `help` | Lists all available MCP commands |
| `ingestion_help` | Detailed ingestion workflow with examples |
| `dotnet_help` | .NET-specific documentation generation guide |
| `rust_help` | Rust-specific rustdoc JSON generation guide |

### Ingestion
| Tool | Required Params | Payload |
|---|---|---|
| `ingest_csharp_xml` | `solution`, `project_id` | `xml` or `xml_path` |
| `ingest_rustdoc_json` | `solution`, `project_id` | `json` or `json_path` |

### Metadata
| Tool | Required Params | Optional |
|---|---|---|
| `list_solutions` | _(none)_ | |
| `list_projects` | `solution` | `limit` |
| `search_projects` | `solution`, `pattern` | `limit` |
| `list_ingests` | `solution`, `project_id` | `limit` |
| `get_ingest` | `solution`, `ingest_id` | |
| `delete_solution` | `solution`, `confirm=true` | _destructive: deletes the whole solution database_ |
| `list_doc_sources` | `solution`, `project_id` | `ingest_id`, `limit` |
| `get_doc_source` | `solution`, `doc_source_id` | |

### Data
| Tool | Required Params | Optional |
|---|---|---|
| `list_symbol_types` | `solution`, `project_id` | |
| `get_members` | `solution`, `project_id`, `scope` | `limit` |
| `get_symbol` | `solution`, `project_id`, `symbol_key` | |
| `list_doc_blocks` | `solution`, `project_id`, `symbol_key` | `ingest_id` |
| `get_symbol_adjacency` | `solution`, `project_id`, `symbol_key` | `limit` |
| `search_symbols` | `solution`, `project_id`, `name` | `limit` |
| `search_symbols_advanced` | `solution`, `project_id` | `name`, `qualified_name`, `symbol_key`, `signature`, `limit` |
| `search_doc_blocks` | `solution`, `project_id`, `text` | `limit` |
| `audit_project_completeness` | `solution`, `project_id` | |
