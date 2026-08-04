#!/usr/bin/env python3
"""md2pdf — Markdown -> PDF or HTML via the @local/md2pdf Typst package.

All Markdown processing lives in the Typst package. This script only does the
host I/O Typst's compile sandbox cannot do itself: it discovers remote image
URLs (pass 1), fetches them, and renders (pass 2). It contains no Markdown
logic.

Standard library only, Python 3.9+, and no behaviour that differs per OS
beyond one documented symlink fallback.

    md2pdf.py [--html|--pdf] <input.md> [output]
"""

import argparse
import ipaddress
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

MIN_TYPST = (0, 15)

# Stands in for an image that could not be fetched, so one bad URL degrades to
# a blank box instead of failing the whole render.
PLACEHOLDER = b'<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>'

# The document is rendered from a file written next to it, so that `read()` and
# `image()` resolve against the document's own directory rather than the
# package root. `asset` and `read-asset` are the same trick for images the
# template places itself (header/footer, cover logo) and for the HTML target,
# which embeds every image as a data: URI.
MAIN_TEMPLATE = """#import "@local/md2pdf:0.1.0": prepare, prepare-html
#let _src = read("{docname}")
#let _html = sys.inputs.at("md2pdf-target", default: "") == "html"
#let _d = prepare(_src, asset: (p, ..a) => image(p, ..a))
#metadata(_d.remotes) <md2pdf-remote-images>
#if _html [
  #metadata(prepare-html(_src, read-asset: p => read(p, encoding: none))) <md2pdf-html>
] else if not _d.skip {{
  show: _d.template
  eval(_d.body, mode: "markup", scope: _d.scope)
}}
"""


def die(message):
    print("md2pdf: " + message, file=sys.stderr)
    raise SystemExit(1)


def warn(message):
    print("md2pdf: WARN " + message, file=sys.stderr)


# ---------------------------------------------------------------------------
# Typst
# ---------------------------------------------------------------------------


def find_typst():
    """The `typst` binary, checked for a new enough version.

    `shutil.which` picks up `typst.exe` on Windows without special-casing.
    """
    exe = shutil.which("typst")
    if not exe:
        die("`typst` not found on PATH — install Typst %d.%d or newer" % MIN_TYPST)
    try:
        out = subprocess.run(
            [exe, "--version"], capture_output=True, text=True, check=True
        ).stdout
    except (OSError, subprocess.CalledProcessError) as e:
        die("could not run `typst --version`: %s" % e)
    # "typst 0.15.1 (9dfd3a08)"
    parts = out.split()
    version = (0, 0)
    if len(parts) >= 2:
        try:
            nums = parts[1].split(".")
            version = (int(nums[0]), int(nums[1]))
        except (ValueError, IndexError):
            pass
    if version < MIN_TYPST:
        die(
            "Typst %d.%d or newer required (found %s) — the HTML target needs "
            "`typst eval`" % (MIN_TYPST[0], MIN_TYPST[1], out.strip() or "unknown")
        )
    return exe


def typst_base(docdir, fonts):
    return [
        "--root",
        str(docdir),
        "--font-path",
        str(fonts),
        "--ignore-system-fonts",
    ]


def typst_eval(exe, expression, main, docdir, fonts, inputs):
    """Evaluate an expression in the document's context and parse the result.

    Replaces the `typst query --field value` of the old shell shim, which Typst
    0.15 deprecates.
    """
    cmd = [exe, "eval", expression, "--in", str(main), "--format", "json"]
    cmd += typst_base(docdir, fonts)
    for key, value in inputs.items():
        cmd += ["--input", "%s=%s" % (key, value)]
    done = subprocess.run(cmd, capture_output=True, text=True)
    if done.returncode != 0:
        return None, done.stderr
    try:
        return json.loads(done.stdout), None
    except json.JSONDecodeError as e:
        return None, str(e)


# ---------------------------------------------------------------------------
# Remote images
# ---------------------------------------------------------------------------


class SafeRedirect(urllib.request.HTTPRedirectHandler):
    """Follow redirects only while they stay http(s) and pass the host check.

    urllib follows redirects by default with no scheme check at all, so without
    this a remote server could bounce a fetch to `file:///etc/passwd` or to an
    address the host policy rejects.
    """

    max_repeats = 5
    max_redirections = 5

    def __init__(self, allow_private):
        self.allow_private = allow_private

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        check_url(newurl, self.allow_private)
        return urllib.request.HTTPRedirectHandler.redirect_request(
            self, req, fp, code, msg, headers, newurl
        )


def check_url(url, allow_private):
    """Reject anything that is not a plain http(s) fetch of a public host.

    The address check is best-effort: it resolves the name and inspects the
    answers, so a server that re-resolves to a private address between this
    check and the connection could still slip through. Closing that needs
    connection-level pinning, which urllib does not expose.
    """
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme not in ("http", "https"):
        raise ValueError("refusing %s: only http and https are fetched" % parsed.scheme)
    host = parsed.hostname
    if not host:
        raise ValueError("no host in URL")
    if allow_private:
        return
    try:
        infos = socket.getaddrinfo(host, parsed.port or (443 if parsed.scheme == "https" else 80))
    except socket.gaierror as e:
        raise ValueError("cannot resolve %s: %s" % (host, e))
    for info in infos:
        addr = ipaddress.ip_address(info[4][0])
        if (
            addr.is_private
            or addr.is_loopback
            or addr.is_link_local
            or addr.is_reserved
            or addr.is_multicast
        ):
            raise ValueError(
                "refusing %s: resolves to the non-public address %s "
                "(use --allow-private-hosts to permit it)" % (host, addr)
            )


def fetch(url, dest, timeout, max_bytes, allow_private):
    """Download one image, streaming so an oversized body is cut off early."""
    check_url(url, allow_private)
    opener = urllib.request.build_opener(SafeRedirect(allow_private))
    request = urllib.request.Request(url, headers={"User-Agent": "md2pdf"})
    with opener.open(request, timeout=timeout) as response:
        size = 0
        with open(dest, "wb") as out:
            while True:
                chunk = response.read(64 * 1024)
                if not chunk:
                    break
                size += len(chunk)
                if size > max_bytes:
                    raise ValueError(
                        "larger than the %d MB limit" % (max_bytes // (1024 * 1024))
                    )
                out.write(chunk)


def fetch_all(items, into, args):
    """Fetch every distinct remote image into `into`, keyed by its alias.

    Nothing is cached between runs: a shared cache keyed by a hash of the URL
    would let one document read another's downloads, since the alias is
    derivable from the URL alone.
    """
    seen = {}
    for item in items:
        url, alias = item.get("url"), item.get("alias")
        if not url or not alias:
            continue
        name = Path(alias).name
        if url in seen:
            # Same image twice in one document — link the second alias at the
            # bytes already on disk rather than fetching again.
            if seen[url] != name:
                shutil.copyfile(into / seen[url], into / name)
            continue
        try:
            fetch(url, into / name, args.timeout, args.max_size * 1024 * 1024, args.allow_private_hosts)
            seen[url] = name
            print("md2pdf: fetched %s" % url)
        except (urllib.error.URLError, ValueError, OSError, socket.timeout) as e:
            (into / name).write_bytes(PLACEHOLDER)
            warn("could not fetch %s: %s" % (url, e))


def expose(docdir, tmpdir):
    """Make the downloads visible to Typst as `<docdir>/remote`.

    Typst reads only below `--root` but follows symlinks out of it, so a link
    is enough and copies nothing. Windows restricts symlink creation to
    Developer Mode or an elevated process, so fall back to a real directory.
    Returns a callable that undoes whichever was used.
    """
    link = docdir / "remote"
    if link.is_symlink():
        link.unlink()
    elif link.exists():
        die("%s is in the way — delete it (md2pdf needs that name)" % link)
    try:
        os.symlink(str(tmpdir), str(link), target_is_directory=True)
        return lambda: link.unlink(missing_ok=True)
    except (OSError, NotImplementedError, AttributeError):
        shutil.copytree(str(tmpdir), str(link))
        return lambda: shutil.rmtree(str(link), ignore_errors=True)


# ---------------------------------------------------------------------------


def parse_args(argv):
    p = argparse.ArgumentParser(
        prog="md2pdf",
        description="Render Markdown to PDF or to one self-contained HTML file.",
    )
    p.add_argument("input", type=Path, help="the Markdown file")
    p.add_argument("output", type=Path, nargs="?", help="defaults to <input>.pdf")
    fmt = p.add_mutually_exclusive_group()
    fmt.add_argument("--html", dest="format", action="store_const", const="html")
    fmt.add_argument("--pdf", dest="format", action="store_const", const="pdf")
    p.add_argument(
        "--timeout", type=float, default=20.0, metavar="SEC",
        help="per remote image, default 20",
    )
    p.add_argument(
        "--max-size", type=int, default=32, metavar="MB",
        help="reject a remote image larger than this, default 32",
    )
    p.add_argument(
        "--allow-private-hosts", action="store_true",
        help="permit remote images on private or link-local addresses",
    )
    args = p.parse_args(argv)

    if not args.input.is_file():
        p.error("no such file: %s" % args.input)
    # Without an explicit flag the output extension picks the format.
    if args.format is None:
        suffix = args.output.suffix.lower() if args.output else ""
        args.format = "html" if suffix in (".html", ".htm") else "pdf"
    if args.output is None:
        args.output = args.input.with_suffix("." + args.format)
    return args


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    exe = find_typst()

    # `Path.resolve()` walks the symlink chain, so the fonts are found even when
    # the script is reached through a link on PATH.
    here = Path(__file__).resolve().parent
    fonts = here.parent / "fonts"
    docdir = args.input.resolve().parent
    main_typ = docdir / (".md2pdf-main-%d.typ" % os.getpid())
    tmpdir = Path(tempfile.mkdtemp(prefix="md2pdf-"))
    undo_remote = None

    try:
        main_typ.write_text(
            MAIN_TEMPLATE.format(docname=args.input.name), encoding="utf-8"
        )

        # Pass 1 — discover remote image URLs, which the Typst sandbox cannot
        # fetch itself.
        remotes, err = typst_eval(
            exe,
            "query(<md2pdf-remote-images>).first().value",
            main_typ, docdir, fonts, {"md2pdf-query": "1"},
        )
        if err:
            remotes = []
        fetch_all(remotes or [], tmpdir, args)
        undo_remote = expose(docdir, tmpdir)

        # Pass 2 — render.
        if args.format == "html":
            # The engine renders the HTML itself and returns it as a metadata
            # value, so the CLI and the browser emit the same bytes.
            # `md2pdf-query=1` short-circuits prepare(), so the PDF body is
            # never built.
            html, err = typst_eval(
                exe,
                "query(<md2pdf-html>).first().value",
                main_typ, docdir, fonts,
                {"md2pdf-query": "1", "md2pdf-target": "html"},
            )
            if err or not isinstance(html, str):
                die("HTML render failed:\n%s" % (err or "unexpected result"))
            # newline="" keeps Windows from rewriting the engine's line endings.
            with open(args.output, "w", encoding="utf-8", newline="") as out:
                out.write(html)
        else:
            cmd = [exe, "compile"] + typst_base(docdir, fonts)
            cmd += [str(main_typ), str(args.output)]
            if subprocess.run(cmd).returncode != 0:
                raise SystemExit(1)
    finally:
        if undo_remote:
            undo_remote()
        main_typ.unlink(missing_ok=True)
        shutil.rmtree(str(tmpdir), ignore_errors=True)

    print("md2pdf: wrote %s" % args.output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
