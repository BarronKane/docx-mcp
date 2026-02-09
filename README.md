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
      "url": "http://127.0.0.1:4020/mcp",
      "type": "streamableHttp"
    }
  }
}
```
