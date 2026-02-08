#!/usr/bin/env python3
"""MCP stdio inspector for docx-mcpd.

This is a lightweight smoke test that speaks MCP JSON-RPC over stdio. It can
list tools and call the health tool to validate the server is responding.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import sys
from pathlib import Path
from typing import Any

try:
    import mcp_inspector
except ImportError as exc:  # pragma: no cover - run-time dependency check
    raise SystemExit(
        "mcp-inspector is required. Install it with: "
        "python -m pip install -r requirements.txt"
    ) from exc

DEFAULT_PROTOCOL_VERSION = "2025-03-26"
DEFAULT_CLIENT_NAME = "docx-mcp-inspect"
DEFAULT_CLIENT_VERSION = "0.1.0"


def default_command() -> list[str]:
    exe = "docx-mcpd.exe" if os.name == "nt" else "docx-mcpd"
    candidate = Path("target") / "debug" / exe
    if candidate.exists():
        return [str(candidate)]
    return ["cargo", "run", "-q", "-p", "docx-mcpd"]


class JsonRpcClient:
    def __init__(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
        timeout: float,
        show_raw: bool,
    ) -> None:
        self._reader = reader
        self._writer = writer
        self._timeout = timeout
        self._show_raw = show_raw
        self._next_id = 1

    async def request(self, method: str, params: dict[str, Any] | None) -> dict[str, Any]:
        request_id = self._next_id
        self._next_id += 1
        payload: dict[str, Any] = {"jsonrpc": "2.0", "id": request_id, "method": method}
        if params is not None:
            payload["params"] = params
        await self._send(payload)
        while True:
            message = await self._read_message()
            if message.get("id") == request_id:
                return message

    async def notify(self, method: str, params: dict[str, Any] | None = None) -> None:
        payload: dict[str, Any] = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            payload["params"] = params
        await self._send(payload)

    async def _send(self, payload: dict[str, Any]) -> None:
        data = json.dumps(payload, separators=(",", ":")) + "\n"
        self._writer.write(data.encode("utf-8"))
        await self._writer.drain()

    async def _read_message(self) -> dict[str, Any]:
        while True:
            try:
                line = await asyncio.wait_for(self._reader.readline(), timeout=self._timeout)
            except asyncio.TimeoutError as exc:
                raise RuntimeError("timed out waiting for MCP response") from exc

            if not line:
                raise RuntimeError("MCP server closed the connection")

            raw = line.decode("utf-8", errors="replace").strip()
            if not raw:
                continue

            try:
                return json.loads(raw)
            except json.JSONDecodeError:
                if self._show_raw:
                    print(f"[non-json] {raw}", file=sys.stderr)
                continue


def extract_tool_names(response: dict[str, Any]) -> list[str]:
    result = response.get("result", {})
    tools = result.get("tools", []) if isinstance(result, dict) else []
    names: list[str] = []
    for tool in tools:
        if isinstance(tool, dict):
            name = tool.get("name")
        else:
            name = getattr(tool, "name", None)
        if name:
            names.append(name)
    return names


def extract_health_text(response: dict[str, Any]) -> str | None:
    result = response.get("result", {})
    if not isinstance(result, dict):
        return None
    content = result.get("content", [])
    if not isinstance(content, list):
        return None
    for item in content:
        if isinstance(item, dict) and item.get("type") == "text":
            text = item.get("text")
            if isinstance(text, str):
                return text
    return None


def extract_capability_names(response: dict[str, Any]) -> list[str]:
    result = response.get("result", {})
    if not isinstance(result, dict):
        return []
    capabilities = result.get("capabilities", {})
    if not isinstance(capabilities, dict):
        return []
    names = [name for name, value in capabilities.items() if value is not None]
    return names


async def drain_stderr(stream: asyncio.StreamReader) -> None:
    while True:
        line = await stream.readline()
        if not line:
            return
        sys.stderr.write(line.decode("utf-8", errors="replace"))


async def run_inspection(args: argparse.Namespace) -> int:
    command = args.command or default_command()
    cmd = command[0]
    cmd_args = command[1:] + args.args

    if cmd == "cargo":
        print("note: using cargo; build output on stdout will be ignored", file=sys.stderr)

    proc = await asyncio.create_subprocess_exec(
        cmd,
        *cmd_args,
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
        env=build_env(args.env),
    )

    assert proc.stdin is not None
    assert proc.stdout is not None
    assert proc.stderr is not None

    stderr_task = None
    if args.show_stderr:
        stderr_task = asyncio.create_task(drain_stderr(proc.stderr))

    client = JsonRpcClient(proc.stdout, proc.stdin, args.timeout, args.show_raw)
    try:
        init_params = {
            "protocolVersion": args.protocol,
            "capabilities": {},
            "clientInfo": {"name": DEFAULT_CLIENT_NAME, "version": DEFAULT_CLIENT_VERSION},
        }
        init_response = await client.request("initialize", init_params)
        await client.notify("notifications/initialized")

        server_info = init_response.get("result", {}).get("serverInfo", {})
        if isinstance(server_info, dict):
            name = server_info.get("name") or "<unknown>"
            version = server_info.get("version") or "<unknown>"
            print(f"server: {name} ({version})")
        else:
            print("server: <unknown>")

        inspector_version = getattr(mcp_inspector, "__version__", "unknown")
        print(f"inspector: mcp-inspector {inspector_version}")

        capability_names = extract_capability_names(init_response)
        print(f"capabilities ({len(capability_names)}): {', '.join(capability_names)}")

        tools_response = await client.request("tools/list", {})
        tool_names = extract_tool_names(tools_response)
        print(f"tools ({len(tool_names)}): {', '.join(tool_names)}")

        health_response = await client.request(
            "tools/call",
            {"name": "health", "arguments": {}},
        )
        health_text = extract_health_text(health_response)
        print(f"health: {health_text or '<no text>'}")

        if args.check:
            required_capabilities = args.require_capability or []
            missing_capabilities = [
                capability
                for capability in required_capabilities
                if capability not in capability_names
            ]
            if missing_capabilities:
                print(
                    f"missing capabilities: {', '.join(missing_capabilities)}",
                    file=sys.stderr,
                )
                return 1
            required = args.require_tool or ["health"]
            missing = [tool for tool in required if tool not in tool_names]
            if missing:
                print(f"missing tools: {', '.join(missing)}", file=sys.stderr)
                return 2
            if health_text != "ok":
                print("health tool did not return ok", file=sys.stderr)
                return 3

        return 0
    finally:
        if proc.stdin:
            proc.stdin.close()
        proc.terminate()
        try:
            await asyncio.wait_for(proc.wait(), timeout=5)
        except asyncio.TimeoutError:
            proc.kill()
        if stderr_task:
            stderr_task.cancel()


def build_env(overrides: list[str]) -> dict[str, str]:
    env = dict(os.environ)
    for item in overrides:
        if "=" not in item:
            continue
        key, value = item.split("=", 1)
        env[key] = value
    return env


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Inspect the docx MCP server over stdio.")
    parser.add_argument(
        "--command",
        nargs="+",
        help="Server command and arguments (defaults to target/debug/docx-mcpd).",
    )
    parser.add_argument(
        "--args",
        nargs=argparse.REMAINDER,
        default=[],
        help="Extra args appended to --command.",
    )
    parser.add_argument(
        "--env",
        action="append",
        default=[],
        help="Environment override in KEY=VALUE form (repeatable).",
    )
    parser.add_argument(
        "--protocol",
        default=DEFAULT_PROTOCOL_VERSION,
        help="MCP protocol version to advertise.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=10.0,
        help="Seconds to wait for MCP responses.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit non-zero if required tools are missing or health fails.",
    )
    parser.add_argument(
        "--require-tool",
        action="append",
        default=[],
        help="Tool name to require when --check is set (repeatable).",
    )
    parser.add_argument(
        "--require-capability",
        action="append",
        default=[],
        help="Capability name to require when --check is set (repeatable).",
    )
    parser.add_argument(
        "--show-raw",
        action="store_true",
        help="Print non-JSON stdout lines from the server.",
    )
    parser.add_argument(
        "--show-stderr",
        action="store_true",
        help="Stream server stderr to this process.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command is None:
        args.command = default_command()
    return asyncio.run(run_inspection(args))


if __name__ == "__main__":
    raise SystemExit(main())
