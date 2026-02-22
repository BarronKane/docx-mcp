#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

# Load .env from the scripts directory if present.
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [[ -f "${SCRIPT_DIR}/.env" ]]; then
    set -a
    # shellcheck source=/dev/null
    source "${SCRIPT_DIR}/.env"
    set +a
fi

DOCX_DB_URI="${DOCX_DB_URI:-ws://127.0.0.1:8000}"
DOCX_DB_USERNAME="${DOCX_DB_USERNAME:-root}"
DOCX_DB_PASSWORD="${DOCX_DB_PASSWORD:-root}"

# Derive the health check URL from the DB URI (ws:// -> http://).
DOCX_SURREAL_HEALTH_URL="${DOCX_DB_URI/ws:\/\//http://}/health"

if ! curl -fsS "${DOCX_SURREAL_HEALTH_URL}" >/dev/null; then
    echo "SurrealDB is not reachable at ${DOCX_SURREAL_HEALTH_URL}" >&2
    echo "Start your SurrealDB container first, then rerun this script." >&2
    exit 1
fi

export DOCX_DB_IN_MEMORY=0
export DOCX_DB_URI
export DOCX_DB_USERNAME
export DOCX_DB_PASSWORD
export DOCX_MCP_SERVE="${DOCX_MCP_SERVE:-1}"
export DOCX_INGEST_SERVE="${DOCX_INGEST_SERVE:-1}"
export DOCX_ENABLE_STDIO="${DOCX_ENABLE_STDIO:-0}"
export DOCX_TEST="${DOCX_TEST:-0}"

echo "Launching docx-mcpd (debug) with SurrealDB at ${DOCX_DB_URI}"

exec cargo run -p docx-mcpd --bin docx-mcpd -- "$@"
