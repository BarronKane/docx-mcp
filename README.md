# docx-mcp

Docx MCP workspace with a daemon and supporting libraries for indexing and serving docx metadata.

Crates:
- docx-mcpd: MCP daemon
- docx-mcp: MCP tools
- docx-core: core types and helpers
- docx-store: storage models and schema helpers

Testing:

`npx @modelcontextprotocol/inspector -- target\debug\docx-mcpd.exe`

When running locally you can add it to an mcp json:

```json
{
  "mcpServers": {
    "docx-mcp": {
      "command": "cmd",
      "args": [
        "/C",
        "docker",
        "run",
        "--rm",
        "-i",
        "--network",
        "docx-mcp_default",
        "-e",
        "DOCX_ENABLE_STDIO=1",
        "-e",
        "DOCX_MCP_SERVE=0",
        "-e",
        "DOCX_INGEST_SERVE=0",
        "-e",
        "DOCX_DB_IN_MEMORY=0",
        "-e",
        "DOCX_DB_URI=ws://surrealdb:8000",
        "-e",
        "DOCX_DB_USERNAME=root",
        "-e",
        "DOCX_DB_PASSWORD=root",
        "barronkane/docx-mcp:latest",
        "/usr/local/bin/docx-mcpd"
      ]
    }
  }
}
```

Minimal SurrealDB compose file (fits the `mcp.json` entry above):

```yaml
services:
  surrealdb:
    image: surrealdb/surrealdb:latest
    command:
      - start
      - --bind
      - 0.0.0.0:8000
      - -u
      - root
      - -p
      - root
      - memory
```

Start it with:

```bash
docker compose -f surrealdb.compose.yaml up -d
```

Notes:
- JetBrains HTTP MCP is currently unreliable with `docx-mcp` (streamable HTTP handshake/session requirements). Use stdio via `docker run` as shown above.
- If you use compose, the default network is usually `<folder>_default` (for this repo: `docx-mcp_default`).

## Docker

Build the image:

```bash
docker build -t docx-mcp:local .
```

Run with stdio (default):

```bash
docker run --rm -it docx-mcp:local
```

Enable HTTP MCP + ingest (streamable HTTP):

```bash
docker run --rm -p 4020:4020 -p 4010:4010 \
  -e DOCX_MCP_SERVE=1 \
  -e DOCX_INGEST_SERVE=1 \
  docx-mcp:local
```

Or use compose (stdio by default, HTTP disabled):

```bash
docker compose up --build
```

Container defaults:
- MCP HTTP: disabled (`DOCX_MCP_SERVE=0`).
- Ingest HTTP: disabled (`DOCX_INGEST_SERVE=0`).
- Stdio: enabled (`DOCX_ENABLE_STDIO=1`).
- External DB client: enabled (`DOCX_DB_IN_MEMORY=0`).

Notes:
- Compose runs SurrealDB as a separate service and wires `DOCX_DB_URI=ws://surrealdb:8000` by default.
- When running the container directly, provide your own `DOCX_DB_URI` + credentials.
- When `DOCX_MCP_SERVE=0`, a non-memory database is required unless `--test` is supplied (set `DOCX_DB_IN_MEMORY=0` with `DOCX_DB_URI` + credentials).

Override addresses with:
- `DOCX_MCP_HTTP_ADDR`
- `DOCX_INGEST_ADDR`

Override SurrealDB settings in compose via the `surrealdb` service definition.
