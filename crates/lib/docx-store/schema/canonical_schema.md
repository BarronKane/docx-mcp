# Canonical Docx Store Schema

This schema is designed for one SurrealDB database per solution. Every record that represents
ingested content carries a `project_id` so queries can filter by the project (C# project or
Rust crate) inside the solution.

## Core tables

- `project`: One row per project or crate within the solution.
- `ingest`: One row per ingestion run. This captures version metadata (git commit, branch,
  tag, project version, and observed modified time).
- `doc_source`: One row per input source file (for example, a C# XML doc file).
- `symbol`: Canonical symbol records (methods, types, fields, etc). `symbol.kind` is a free
  string and can vary by language.
- `doc_block`: Normalized documentation content per symbol and ingest.
- `doc_chunk`: Optional chunked text for retrieval or embeddings.

## Key fields

- `project.project_id`: Stable project identifier in the solution.
- `symbol.symbol_key`: Canonical symbol ID. Recommended format:
  `{language}|{project_id}|{source_id}`.
- `doc_block.doc_hash`: Optional hash for dedupe across ingests.
- `ingest.*`: `git_commit`, `git_branch`, `git_tag`, `project_version`,
  `source_modified_at`, `ingested_at`.

## Relationships

Graph edges are stored as relation tables (for example, `contains`, `member_of`,
`documents`, `references`, `see_also`, `inherits`, `implements`). All relations
include `project_id` and optional `ingest_id` for version filtering.

## Dynamic symbol kind

`symbol.kind` is intentionally free-form. The ingest pipeline should record whatever
the source provides (for example, `class`, `trait`, `method`), and AI clients can
interpret it by language at query time.

## Dedupe strategy

When ingesting, compute a `doc_hash` over normalized doc content. If a new
`doc_block` has the same `doc_hash` for the same `project_id` and `symbol_key`,
it can be skipped or linked to the new `ingest` without duplication.

## Files in this folder

- `surrealdb.surql`: Draft SurrealDB schema (tables, fields, indexes).
- `dotnet_xml_mapping.md`: Mapping spec for .NET XML docs.
