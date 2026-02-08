# Rustdoc JSON Mapping

This mapping targets the JSON output produced by `cargo doc` with
`--output-format json`. Unlike .NET XML, rustdoc emits the full crate surface
area, so the parser filters to the root crate and only creates doc blocks when
documentation is present.

## Symbol identity

- Rustdoc item `id` becomes the stable source ID.
- `symbol.symbol_key = "rust|{project_id}|{qualified_path}"`.
- `symbol.kind` maps from rustdoc item kind (`module`, `struct`, `enum`,
  `trait`, `function`, `type_alias`, `const`, `static`, `union`, `macro`,
  `field`, `variant`, `method`, `trait_item`).
- `symbol.source_ids`: add `{ kind: "rustdoc_id", value: item_id }`.

## Basic fields

- `symbol.name`: rustdoc item name.
- `symbol.qualified_name`: module-qualified name (crate root included).
- `symbol.signature`: formatted from function inputs/output when available.
- `symbol.visibility`: rustdoc `visibility` string.
- `symbol.is_async`, `symbol.is_const`, `symbol.is_static`: derived from item headers.
- `symbol.source_path`, `symbol.line`, `symbol.col`: from rustdoc `span`.

## Doc block mapping

Rustdoc `docs` is markdown. The parser splits the preamble into summary and
remarks, and then maps headings to fields:

- `# Errors` -> `doc_block.errors`
- `# Panics` -> `doc_block.panics`
- `# Safety` -> `doc_block.safety`
- `# Returns` -> `doc_block.returns`
- `# Value` -> `doc_block.value`
- `# Deprecated` -> `doc_block.deprecated`
- `# Examples` -> `doc_block.examples[]` (code fences or text body)
- `# Notes` -> `doc_block.notes[]`
- `# Warnings` -> `doc_block.warnings[]`
- `# Parameters`/`# Arguments` -> `doc_block.params[]` (bullet list parsing)
- `# Type Parameters` -> `doc_block.type_params[]` (bullet list parsing)

Unrecognized headings are preserved as `doc_block.sections[]`.

## Relationships

- `documents` edge from `doc_block` to `symbol`.
- Additional edges (e.g., `member_of`, `contains`) can be inferred from
  qualified names or impl ownership if desired.

## Versioning

Each `doc_block` references the ingest run:

- `doc_block.ingest_id = ingest.id`
- `doc_block.source_kind = "rustdoc_json"`
- `doc_block.language = "rust"`

## Filtering

- Only items from the root crate (`crate_id` matching the root module) are
  ingested. External crate items are skipped.
- Doc blocks are only created when `docs` content is non-empty.
