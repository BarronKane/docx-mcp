import os
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "mcp_inspect.py"


class McpInspectorTest(unittest.TestCase):
    def test_stdio_health(self) -> None:
        env = dict(os.environ)
        env.setdefault("DOCX_MCP_HTTP_ADDR", "127.0.0.1:0")
        env.setdefault("DOCX_INGEST_ADDR", "127.0.0.1:0")

        try:
            build = subprocess.run(
                ["cargo", "build", "-q", "-p", "docx-mcpd"],
                cwd=ROOT,
                env=env,
                capture_output=True,
                text=True,
                timeout=600,
                check=False,
            )
        except subprocess.TimeoutExpired as exc:
            raise AssertionError("cargo build timed out") from exc

        if build.returncode != 0:
            sys.stderr.write(build.stdout)
            sys.stderr.write(build.stderr)
            raise AssertionError("cargo build failed")

        command = [
            sys.executable,
            str(SCRIPT),
            "--check",
            "--require-capability",
            "tools",
            "--require-tool",
            "health",
            "--require-tool",
            "list_solutions",
        ]

        try:
            result = subprocess.run(
                command,
                cwd=ROOT,
                env=env,
                capture_output=True,
                text=True,
                timeout=180,
                check=False,
            )
        except subprocess.TimeoutExpired as exc:
            raise AssertionError("mcp inspector test timed out") from exc

        if result.returncode != 0:
            sys.stderr.write(result.stdout)
            sys.stderr.write(result.stderr)

        self.assertEqual(result.returncode, 0, "mcp inspector exited non-zero")


if __name__ == "__main__":
    unittest.main()
