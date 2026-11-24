import contextlib
import http.server
import importlib.machinery
import importlib.util
import io
import json
import os
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import zlib
from pathlib import Path
from types import SimpleNamespace
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
            ("md2pdf://syntax", "## Headings and anchors"),
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


    def test_bad_argument_types_answer_with_the_request_id(self):
        for params in (
            {"name": "render_file", "arguments": "str"},
            {"name": "render_file", "arguments": {"input_path": 5}},
            {"name": "render_file", "arguments": {"input_path": "a.md", "output_path": 7}},
        ):
            response, _ = cli.mcp_dispatch(
                {"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": params},
                ROOT,
                False,
            )
            self.assertEqual(response["id"], 9)
            self.assertEqual(response["error"]["code"], -32602)
        response, _ = cli.mcp_dispatch(
            {"jsonrpc": "2.0", "id": 10, "method": "resources/read", "params": {"uri": ["x"]}},
            ROOT,
            False,
        )
        self.assertEqual((response["id"], response["error"]["code"]), (10, -32602))

    def test_notifications_never_get_a_response(self):
        for method in ("notifications/initialized", "notifications/unknown", "ping", "bogus"):
            response, _ = cli.mcp_dispatch({"jsonrpc": "2.0", "method": method}, ROOT, False)
            self.assertIsNone(response, method)

    def test_non_object_message_is_an_invalid_request(self):
        response, _ = cli.mcp_dispatch([1, 2], ROOT, False)
        self.assertEqual(response["error"]["code"], -32600)

    def test_resources_read_reports_missing_docs_instead_of_crashing(self):
        with patch.object(Path, "read_text", side_effect=FileNotFoundError(2, "No such file")):
            response, _ = cli.mcp_dispatch(
                {"jsonrpc": "2.0", "id": 3, "method": "resources/read",
                 "params": {"uri": "md2pdf://syntax"}},
                ROOT,
                False,
            )
        self.assertEqual((response["id"], response["error"]["code"]), (3, -32002))

    def test_render_passes_the_output_explicitly(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "in.md").write_text("# Test", encoding="utf-8")
            completed = subprocess.CompletedProcess([], 0, "", "")
            with patch.object(cli.subprocess, "run", return_value=completed) as run:
                cli.mcp_render(root, {"input_path": "in.md", "output_path": "notes.md"})
            cmd = run.call_args.args[0]
            self.assertEqual(cmd[cmd.index("--output-file") + 1], str(root / "notes.md"))


class ArgumentTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name)
        for name in ("a.md", "b.md", "c.markdown"):
            (self.root / name).write_text("# x", encoding="utf-8")

    def plan(self, *argv):
        args = cli.parse_args([str(self.root / a) if a.endswith(".md") else a for a in argv])
        return [(s.name, o, f) for s, o, f in cli.jobs(args)]

    def test_single_input_defaults_to_pdf_next_to_the_input(self):
        self.assertEqual(self.plan("a.md"), [("a.md", self.root / "a.pdf", "pdf")])

    def test_single_input_with_output_infers_format_from_extension(self):
        out = str(self.root / "x.html")
        self.assertEqual(self.plan("a.md", out), [("a.md", Path(out), "html")])

    def test_output_without_known_extension_uses_flag_or_pdf(self):
        out = str(self.root / "out")
        self.assertEqual(self.plan("a.md", out), [("a.md", Path(out), "pdf")])
        self.assertEqual(self.plan("--html", "a.md", out), [("a.md", Path(out), "html")])

    def test_two_markdown_files_are_a_batch_not_an_output(self):
        self.assertEqual(
            self.plan("a.md", "b.md"),
            [("a.md", self.root / "a.pdf", "pdf"), ("b.md", self.root / "b.pdf", "pdf")],
        )

    def test_input_named_twice_stays_an_output(self):
        jobs = self.plan("--html", "a.md", "a.md")
        self.assertEqual(jobs, [("a.md", self.root / "a.md", "html")])

    def test_outdir_and_both_formats(self):
        out = self.root / "out"
        jobs = self.plan("--pdf", "--html", "-o", str(out), "a.md", "b.md")
        self.assertEqual(
            [(n, o.name, f) for n, o, f in jobs],
            [("a.md", "a.pdf", "pdf"), ("a.md", "a.html", "html"),
             ("b.md", "b.pdf", "pdf"), ("b.md", "b.html", "html")],
        )
        self.assertTrue(all(o.parent == out for _, o, _ in jobs))

    def test_conflicting_arguments_are_usage_errors(self):
        with contextlib.redirect_stderr(io.StringIO()):
            for argv in (
                ["--pdf", "--html", str(self.root / "a.md"), str(self.root / "o")],
                [str(self.root / "missing.md")],
                [str(self.root / "a.md"), str(self.root / "b.md"), str(self.root / "c.markdown")]
                + ["--output-file", str(self.root / "o")],
            ):
                with self.assertRaises(SystemExit) as caught:
                    cli.parse_args(argv)
                self.assertEqual(caught.exception.code, 2)

    def test_help_documents_batch_mode(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), self.assertRaises(SystemExit):
            cli.parse_args(["--help"])
        for word in ("-o", "--outdir", "--pdf", "--html", "input.md [input.md ...]"):
            self.assertIn(word, out.getvalue())


class BatchTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name).resolve()
        for name in ("a.md", "b.md"):
            (self.root / name).write_text("# x", encoding="utf-8")

    def run_main(self, *argv, render=None):
        err = io.StringIO()
        out = io.StringIO()
        with patch.object(cli, "find_typst", return_value="typst"), patch.object(
            cli, "render_file", side_effect=render
        ) as render_file, patch.object(cli.signal, "signal"), contextlib.redirect_stderr(
            err
        ), contextlib.redirect_stdout(out):
            code = cli.main([str(self.root / a) if a.endswith(".md") else a for a in argv])
        return code, render_file, out.getvalue(), err.getvalue()

    def test_a_failing_file_does_not_stop_the_batch(self):
        def render(exe, fonts, args, source, output, fmt):
            if source.name == "a.md":
                raise cli.RenderError("boom")

        code, render_file, out, err = self.run_main("a.md", "b.md", render=render)
        self.assertEqual(code, 1)
        self.assertEqual(render_file.call_count, 2)
        self.assertIn("a.md: boom", err)
        self.assertIn("wrote %s" % (self.root / "b.pdf"), err)

    def test_success_exits_zero_and_reports_on_stderr_only(self):
        code, _, out, err = self.run_main("--html", "a.md", "/dev/stdout")
        self.assertEqual((code, out), (0, ""))
        self.assertIn("wrote /dev/stdout", err)

    def test_two_inputs_may_not_share_an_output(self):
        other = self.root / "sub"
        other.mkdir()
        (other / "a.md").write_text("# y", encoding="utf-8")
        code, render_file, _, err = self.run_main(
            "a.md", str(other / "a.md"), "-o", str(self.root / "out")
        )
        self.assertEqual((code, render_file.call_count), (1, 1))
        self.assertIn("is also the output of", err)

    def test_sigterm_exits_through_the_cleanup(self):
        with patch.object(cli, "find_typst", return_value="typst"), patch.object(
            cli, "render_file"
        ), patch.object(cli.signal, "signal") as install, contextlib.redirect_stderr(io.StringIO()):
            cli.main([str(self.root / "a.md")])
        signum, handler = install.call_args.args
        self.assertEqual(signum, signal.SIGTERM)
        with self.assertRaises(SystemExit):
            handler(signum, None)


class RenderFileTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name).resolve()
        self.source = self.root / "in.md"
        self.source.write_text("# x", encoding="utf-8")
        self.args = SimpleNamespace(
            html_theme="system", system_fonts=True, timeout=5, max_size=1,
            allow_private_hosts=True,
        )

    def render(self, output, fmt="html"):
        cli.render_file("typst", self.root, self.args, self.source, output, fmt)

    def test_refuses_to_overwrite_the_input(self):
        with self.assertRaisesRegex(cli.RenderError, "overwrite the input"):
            self.render(self.root / "." / "in.md")
        self.assertEqual(self.source.read_text(encoding="utf-8"), "# x")

    def test_html_without_remote_images_needs_one_eval_and_creates_the_folder(self):
        output = self.root / "new" / "deep" / "in.html"
        with patch.object(
            cli, "typst_eval",
            return_value=({"remotes": [], "images": [], "cites": [], "html": "<p>hi</p>"}, None),
        ) as evaluate:
            self.render(output)
        self.assertEqual(evaluate.call_count, 1)
        self.assertEqual(output.read_text(encoding="utf-8"), "<p>hi</p>")
        self.assertEqual(sorted(p.name for p in self.root.iterdir()), ["in.md", "new"])

    def test_remote_images_are_fetched_into_a_per_run_directory(self):
        seen = []

        def evaluate(exe, expression, main, docdir, fonts, inputs, system_fonts=True):
            if "md2pdf-fetched" in inputs and inputs["md2pdf-fetched"] == "0":
                remotes = [{"url": "http://x/p.png", "alias": "d/h"}]
                return {"remotes": remotes, "images": [], "cites": [], "html": "old"}, None
            seen.append(inputs["md2pdf-remote-dir"])
            return "<p>new</p>", None

        seen_dir = []
        with patch.object(cli, "typst_eval", side_effect=evaluate), patch.object(
            cli, "fetch_all", side_effect=lambda i, into, a: seen_dir.append(into.name)
        ):
            self.render(self.root / "out.html")
        self.assertEqual(seen, seen_dir)
        self.assertTrue(seen[0].startswith(".md2pdf-"))
        self.assertNotEqual(seen[0], "remote")
        self.assertEqual(sorted(p.name for p in self.root.iterdir()), ["in.md", "out.html"])

    def test_file_names_with_quotes_and_backslashes_are_escaped(self):
        self.assertEqual(cli.typst_string('a"b\\c'), 'a\\"b\\\\c')

    def test_read_only_folder_is_a_clean_error(self):
        os.chmod(self.root, 0o500)
        self.addCleanup(os.chmod, self.root, 0o700)
        if os.access(self.root, os.W_OK):
            self.skipTest("directory permissions are not enforced")
        with self.assertRaisesRegex(cli.RenderError, "cannot write in"):
            self.render(Path(tempfile.gettempdir()) / "md2pdf-ro-test.html")


class TypstOutputTests(unittest.TestCase):
    def test_unknown_font_warnings_are_filtered_with_their_code_frames(self):
        stderr = (
            "warning: unknown font family: noto sans sc\n"
            "    \u250c\u2500 @local/x.typ:1:1\n"
            "    \u2502\n"
            "\n"
            "warning: something else\n"
            "  = hint: keep me\n"
            "\n"
            "error: boom\n"
        )
        kept = cli.without_font_warnings(stderr)
        self.assertNotIn("unknown font", kept)
        self.assertNotIn("x.typ", kept)
        self.assertIn("warning: something else", kept)
        self.assertIn("error: boom", kept)


class MarkupDetectionTests(unittest.TestCase):
    def test_only_clear_markup_is_rejected(self):
        for body in (b"<!DOCTYPE html>", b"\xef\xbb\xbf \n<HTML>", b"<head>", b"<body>", b"<p>x</p>"):
            self.assertTrue(cli.is_markup_page(body, ""), body)
        self.assertTrue(cli.is_markup_page(b"GIF89a", "text/html; charset=utf-8"))
        for body in (b"<svg xmlns=''>", b"<?xml version='1.0'?><svg/>", b"<!-- c --><svg/>",
                     b"\x89PNG", b"RIFF\0\0\0\0AVIF", b"BM\0\0", b"\0\0\1\0"):
            self.assertFalse(cli.is_markup_page(body, "image/x"), body)


class FetchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                if self.path == "/soft404":
                    body = b"<!DOCTYPE html><html><body>not found</body></html>"
                    self.send_response(200)
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                elif self.path == "/png":
                    body = b"\x89PNG\r\n\x1a\n" + b"0" * 16
                    self.send_response(200)
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                elif self.path == "/drip":
                    self.send_response(200)
                    self.end_headers()
                    try:
                        for _ in range(100):
                            self.wfile.write(b"x")
                            self.wfile.flush()
                            time.sleep(0.1)
                    except OSError:
                        pass
                elif self.path == "/garbage":
                    self.wfile.write(b"GARBAGE\r\n\r\n")

        cls.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        cls.base = "http://127.0.0.1:%d" % cls.server.server_port
        threading.Thread(target=cls.server.serve_forever, daemon=True).start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.into = Path(self.dir.name)

    def fetch_all(self, *paths):
        args = SimpleNamespace(timeout=5, max_size=1, allow_private_hosts=True)
        items = [{"url": self.base + p, "alias": "r/%d" % i} for i, p in enumerate(paths)]
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            cli.fetch_all(items, self.into, args)
        return err.getvalue()

    def test_a_real_image_is_stored(self):
        self.fetch_all("/png")
        self.assertTrue((self.into / "0").read_bytes().startswith(b"\x89PNG"))

    def test_an_html_soft_404_becomes_the_placeholder(self):
        err = self.fetch_all("/soft404")
        self.assertIn("not an image", err)
        self.assertEqual((self.into / "0").read_bytes(), cli.PLACEHOLDER)

    def test_a_garbage_response_and_a_url_with_spaces_do_not_crash(self):
        err = self.fetch_all("/garbage", "/a b.png")
        self.assertEqual(err.count("could not fetch"), 2)
        self.assertEqual((self.into / "1").read_bytes(), cli.PLACEHOLDER)

    def test_the_timeout_is_a_total_deadline(self):
        started = time.monotonic()
        with self.assertRaisesRegex(ValueError, "timed out"):
            cli.fetch(self.base + "/drip", self.into / "x", 1, 1 << 20, True)
        self.assertLess(time.monotonic() - started, 3)


def real_package_path():
    if not shutil.which("typst"):
        return None
    configured = os.environ.get("TYPST_PACKAGE_PATH")
    if configured:
        return configured
    pkgs = ROOT / "engine" / "target" / "pkgs"
    return str(pkgs) if pkgs.is_dir() else None


@unittest.skipUnless(real_package_path(), "typst or the md2pdf package is not available")
class RealRenderTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name).resolve()
        self.env = dict(os.environ, TYPST_PACKAGE_PATH=real_package_path())

    def start(self, *argv):
        return subprocess.Popen(
            [sys.executable, str(ROOT / "bin" / "md2pdf"), *map(str, argv)],
            env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )

    def run_cli(self, *argv):
        proc = self.start(*argv)
        out, err = proc.communicate(timeout=120)
        return proc.returncode, out, err

    def test_output_without_extension_and_missing_folder(self):
        (self.root / "in.md").write_text("# Hi\n", encoding="utf-8")
        target = self.root / "new" / "out"
        code, _, err = self.run_cli(self.root / "in.md", target)
        self.assertEqual(code, 0, err)
        self.assertTrue(target.read_bytes().startswith(b"%PDF"))
        self.assertNotIn("unknown font family", err)

    def test_missing_local_image_degrades_with_a_warning(self):
        (self.root / "in.md").write_text("# Hi\n\n![x](gone.png)\n", encoding="utf-8")
        for name in ("out.pdf", "out.html"):
            code, _, err = self.run_cli(self.root / "in.md", self.root / name)
            self.assertEqual(code, 0, err)
            self.assertIn("WARN missing image gone.png", err)
            self.assertTrue((self.root / name).stat().st_size > 0)

    def test_unknown_bibliography_style_falls_back(self):
        md = (
            "---\nbibliography: inline\nbibliography-style: no-such-style\n---\n\n"
            "See [@known].\n\n@misc{known, title = {Known}}\n"
        )
        (self.root / "in.md").write_text(md, encoding="utf-8")
        code, _, err = self.run_cli(self.root / "in.md", self.root / "out.pdf")
        self.assertEqual(code, 0, err)
        self.assertTrue((self.root / "out.pdf").read_bytes().startswith(b"%PDF"))

    def test_unknown_citation_key_renders_with_a_warning(self):
        md = (
            "---\nbibliography: inline\n---\n\nSee [@known] and [@nope].\n\n"
            "@misc{known, title = {Known}}\n"
        )
        (self.root / "in.md").write_text(md, encoding="utf-8")
        for name in ("out.pdf", "out.html"):
            code, _, err = self.run_cli(self.root / "in.md", self.root / name)
            self.assertEqual(code, 0, err)
            self.assertIn("WARN unknown citation key nope in", err)
            self.assertNotIn("key known", err)
        self.assertIn("[@nope]", (self.root / "out.html").read_text(encoding="utf-8"))

    def test_awkward_file_names(self):
        for name in ('q"uote.md', "back\\slash.md"):
            (self.root / name).write_text("# Hi\n", encoding="utf-8")
            code, _, err = self.run_cli("--html", self.root / name, self.root / "out.html")
            self.assertEqual(code, 0, (name, err))
            self.assertIn("<h1", (self.root / "out.html").read_text(encoding="utf-8"))
        self.assertEqual(
            sorted(p.name for p in self.root.iterdir() if p.name.startswith(".")), []
        )

    def test_batch_writes_both_formats_into_outdir(self):
        for name in ("a.md", "b.md"):
            (self.root / name).write_text("# " + name + "\n", encoding="utf-8")
        out = self.root / "out"
        code, _, err = self.run_cli(
            "--pdf", "--html", "-o", out, self.root / "a.md", self.root / "b.md"
        )
        self.assertEqual(code, 0, err)
        self.assertEqual(
            sorted(p.name for p in out.iterdir()), ["a.html", "a.pdf", "b.html", "b.pdf"]
        )

    def test_batch_continues_after_a_failure(self):
        (self.root / "good.md").write_text("# ok\n", encoding="utf-8")
        (self.root / "bad.md").write_text("# ok\n", encoding="utf-8")
        out = self.root / "out"
        out.mkdir()
        (out / "bad.pdf").mkdir()
        code, _, err = self.run_cli("-o", out, self.root / "bad.md", self.root / "good.md")
        self.assertEqual(code, 1)
        self.assertIn("bad.md", err)
        self.assertTrue((out / "good.pdf").is_file())

    def test_html_to_the_input_is_refused(self):
        (self.root / "in.md").write_text("# keep\n", encoding="utf-8")
        code, _, err = self.run_cli("--html", self.root / "in.md", self.root / "in.md")
        self.assertEqual(code, 1)
        self.assertEqual((self.root / "in.md").read_text(encoding="utf-8"), "# keep\n")


@unittest.skipUnless(
    real_package_path() and "remote-dir" in (ROOT / "package" / "lib.typ").read_text(encoding="utf-8"),
    "typst or an engine with the remote-dir argument is not available",
)
class RemoteImageTests(RealRenderTests):
    @classmethod
    def setUpClass(cls):
        def chunk(kind, data):
            body = kind + data
            return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

        pixel = (
            b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(b"\x00\xff\x00\x00"))
            + chunk(b"IEND", b"")
        )

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                self.send_response(200)
                self.send_header("Content-Length", str(len(pixel)))
                self.end_headers()
                self.wfile.write(pixel)

        cls.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        cls.base = "http://127.0.0.1:%d" % cls.server.server_port
        threading.Thread(target=cls.server.serve_forever, daemon=True).start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()

    def test_concurrent_runs_in_one_folder_and_a_user_remote_folder(self):
        (self.root / "remote").mkdir()
        (self.root / "remote" / "mine.txt").write_text("keep", encoding="utf-8")
        procs = []
        for i in range(4):
            doc = self.root / ("d%d.md" % i)
            doc.write_text("![x](%s/img%d.png)\n" % (self.base, i), encoding="utf-8")
            fmt = "--html" if i % 2 else "--pdf"
            procs.append(self.start(fmt, "--allow-private-hosts", doc))
        for proc in procs:
            _, err = proc.communicate(timeout=180)
            self.assertEqual(proc.returncode, 0, err)
        self.assertEqual(
            (self.root / "remote" / "mine.txt").read_text(encoding="utf-8"), "keep"
        )
        self.assertIn("data:image/png", (self.root / "d1.html").read_text(encoding="utf-8"))
        self.assertEqual(
            sorted(p.name for p in self.root.iterdir() if p.name.startswith(".")), []
        )


if __name__ == "__main__":
    unittest.main()
