# docx-mcp

Docx MCP workspace with a daemon and supporting libraries for indexing and serving docx metadata.

Crates:
- docx-mcpd: MCP daemon
- docx-mcp: MCP tools
- docx-core: core types and helpers
- docx-store: storage models and schema helpers

Testing:

`npx @modelcontextprotocol/inspector -- target\debug\docx-mcpd.exe --stdio`

## Docker

Pull the image:

```bash
docker pull barronkane/docx-mcp:latest
```

Run (HTTP MCP + ingest enabled by default):

```bash
docker run --rm -p 4020:4020 -p 4010:4010 \
  -e DOCX_DB_URI=ws://surrealdb:8000 \
  -e DOCX_DB_USERNAME=root \
  -e DOCX_DB_PASSWORD=root \
  barronkane/docx-mcp:latest
```

Endpoints:
- MCP HTTP: `http://127.0.0.1:4020/mcp`
- Ingest HTTP: `http://127.0.0.1:4010/ingest`

Run with stdio (optional):

```bash
docker run --rm -it \
  -e DOCX_DB_URI=ws://surrealdb:8000 \
  -e DOCX_DB_USERNAME=root \
  -e DOCX_DB_PASSWORD=root \
  barronkane/docx-mcp:latest --stdio
```

HTTP ingest payloads accept one of `contents` or `contents_path`.
`contents_path` must point to a file accessible to the server host.

Or use compose (pulls `barronkane/docx-mcp:latest`, includes SurrealDB):

```bash
docker compose up -d
```

Development (local build only, not required for normal usage):

```bash
docker compose -f compose.dev.yaml up --build
```

Container defaults:
- MCP HTTP: enabled (`DOCX_MCP_SERVE=1`).
- Ingest HTTP: enabled (`DOCX_INGEST_SERVE=1`).
- Stdio: disabled (`DOCX_ENABLE_STDIO=0`).
- External DB client: enabled (`DOCX_DB_IN_MEMORY=0`).

Notes:
- Compose runs SurrealDB as a separate service and wires `DOCX_DB_URI=ws://surrealdb:8000` by default.
- When running the container directly, provide your own `DOCX_DB_URI` + credentials.
- When `DOCX_MCP_SERVE=0`, a non-memory database is required unless `--test` is supplied (set `DOCX_DB_IN_MEMORY=0` with `DOCX_DB_URI` + credentials).

Override addresses with:
- `DOCX_MCP_HTTP_ADDR`
- `DOCX_INGEST_ADDR`

Override SurrealDB settings in compose via the `surrealdb` service definition.
