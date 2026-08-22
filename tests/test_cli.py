import importlib.machinery
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parent.parent
loader = importlib.machinery.SourceFileLoader("md2pdf_cli", str(ROOT / "bin" / "md2pdf"))
spec = importlib.util.spec_from_loader(loader.name, loader)
cli = importlib.util.module_from_spec(spec)
loader.exec_module(cli)


class McpTests(unittest.TestCase):
    def test_legacy_initialize_advertises_tools_resources_and_instructions(self):
        response, modern = cli.mcp_dispatch(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {"protocolVersion": "2025-06-18"},
            },
            ROOT,
            False,
        )
        self.assertFalse(modern)
        self.assertEqual(response["result"]["protocolVersion"], "2025-06-18")
        self.assertIn("md2pdf://syntax", response["result"]["instructions"])
        self.assertEqual(set(response["result"]["capabilities"]), {"tools", "resources"})

    def test_modern_discovery_and_lists_carry_cache_and_identity(self):
        response, modern = cli.mcp_dispatch(
            {"jsonrpc": "2.0", "id": 1, "method": "server/discover"}, ROOT, False
        )
        self.assertTrue(modern)
        self.assertEqual(response["result"]["supportedVersions"], ["2026-07-28"])
        self.assertEqual(response["result"]["cacheScope"], "public")
        listed, _ = cli.mcp_dispatch(
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"}, ROOT, modern
        )
        self.assertEqual(listed["result"]["tools"][0]["name"], "render_file")
        self.assertEqual(listed["result"]["cacheScope"], "public")
        self.assertEqual(
            listed["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "md2pdf",
        )

    def test_resources_return_the_canonical_reference_and_example(self):
        for uri, marker in [
            ("md2pdf://syntax", "## Headings and navigation"),
            ("md2pdf://example", "# Welcome"),
        ]:
            response, _ = cli.mcp_dispatch(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "resources/read",
                    "params": {"uri": uri},
                },
                ROOT,
                False,
            )
            self.assertIn(marker, response["result"]["contents"][0]["text"])

    def test_paths_cannot_escape_the_configured_root(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            with self.assertRaisesRegex(ValueError, "outside"):
                cli.mcp_path(root, "../escape.md")

            outside = root.parent / (root.name + "-outside.md")
            outside.write_text("# Outside", encoding="utf-8")
            (root / "linked.md").symlink_to(outside)
            with self.assertRaisesRegex(ValueError, "outside"):
                cli.mcp_path(root, "linked.md", must_exist=True)
            outside.unlink()

    def test_render_file_returns_structured_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "in.md").write_text("# Test", encoding="utf-8")
            completed = subprocess.CompletedProcess([], 0, "md2pdf: wrote out.html\n", "")
            with patch.object(cli.subprocess, "run", return_value=completed) as run:
                response, _ = cli.mcp_dispatch(
                    {
                        "jsonrpc": "2.0",
                        "id": 4,
                        "method": "tools/call",
                        "params": {
                            "name": "render_file",
                            "arguments": {
                                "input_path": "in.md",
                                "output_path": "out.html",
                                "html_theme": "dark",
                            },
                        },
                    },
                    root,
                    False,
                )
            result = response["result"]
            self.assertFalse(result["isError"])
            self.assertEqual(result["structuredContent"]["format"], "html")
            self.assertEqual(result["structuredContent"]["mime_type"], "text/html")
            self.assertIn("--html-theme", run.call_args.args[0])
            self.assertEqual(json.loads(result["content"][0]["text"])["warnings"], [])

    def test_render_rejects_input_as_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "same.md").write_text("# Test", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "must differ"):
                cli.mcp_render(
                    root,
                    {"input_path": "same.md", "output_path": "same.md", "format": "html"},
                )

    def test_unknown_tools_return_protocol_errors(self):
        response, _ = cli.mcp_dispatch(
            {
                "jsonrpc": "2.0",
                "id": 7,
                "method": "tools/call",
                "params": {"name": "unknown"},
            },
            ROOT,
            False,
        )
        self.assertEqual(response["error"]["code"], -32602)


if __name__ == "__main__":
    unittest.main()
