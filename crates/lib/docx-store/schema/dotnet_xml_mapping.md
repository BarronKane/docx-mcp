# .NET XML Doc Mapping

This mapping targets the XML documentation file produced by the C# compiler.
Each `<member name="...">` entry maps to one `symbol` and one `doc_block`.

## Symbol identity

- XML `member/@name` becomes the stable source ID.
- `symbol.symbol_key = "csharp|{project_id}|{member_name}"`.
- `symbol.kind` is derived from the prefix:
  - `T:` type, `M:` method, `P:` property, `F:` field, `E:` event, `N:` namespace.

## Basic fields

- `symbol.name`: unqualified simple name from the member.
- `symbol.qualified_name`: namespace-qualified name.
- `symbol.signature`: include parameter types and generic arity when available.
- `symbol.source_ids`: add `{ kind: "csharp_doc_id", value: member_name }`.

## Doc block mapping

XML element -> `doc_block` field

- `<summary>` -> `summary`
- `<remarks>` -> `remarks`
- `<returns>` -> `returns`
- `<value>` -> `value`
- `<param name="x">` -> `params[]`
- `<typeparam name="T">` -> `type_params[]`
- `<exception cref="...">` -> `exceptions[]`
- `<example>` -> `examples[]`
- `<seealso cref="...">` -> `see_also[]`
- `<see cref="...">` -> `see_also[]` or inline link in `summary`/`remarks`
- `<inheritdoc>` -> `inherit_doc`
- `<deprecated>` -> `deprecated`

Inline tags (`<see>`, `<paramref>`, `<typeparamref>`, `<code>`, `<list>`) should be
rendered to markdown for `summary`/`remarks` or captured in `raw`.

## Relationships

- `documents` edge from `doc_block` to `symbol`.
- `references`/`see_also` edges for resolvable `cref` values.
- `member_of`/`contains` edges can be inferred using symbol name structure.

## Versioning

Each `doc_block` should reference the ingest run:

- `doc_block.ingest_id = ingest.id`
- `doc_block.source_kind = "csharp_xml"`
- `doc_block.language = "csharp"`

## Deduping

Compute a `doc_hash` over the normalized `doc_block` content. If the hash is unchanged
for the same `project_id` + `symbol_key`, skip inserting a new `doc_block` for that ingest.
