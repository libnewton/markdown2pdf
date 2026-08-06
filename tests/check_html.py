#!/usr/bin/env python3
"""Assert the properties a rendered md2pdf HTML file has to have.

The engine's unit tests cover the markup it produces; this checks the artefact
that actually ships — after Typst has read the assets and the data: URIs are in
place. Standard library only, Python 3.9+.

    python3 tests/check_html.py out.html [more.html ...]

Every check reports what failed and why rather than asserting bare, because a
line number alone does not say which invariant broke.
"""

import re
import sys
from html.parser import HTMLParser
from pathlib import Path

AA = 4.5  # WCAG 2.1 contrast minimum for normal-size text.


def luminance(hex_colour):
    def channel(pair):
        v = int(pair, 16) / 255
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4

    h = hex_colour.lstrip("#")
    return (
        0.2126 * channel(h[0:2]) + 0.7152 * channel(h[2:4]) + 0.0722 * channel(h[4:6])
    )


def contrast(a, b):
    hi, lo = sorted((luminance(a), luminance(b)), reverse=True)
    return (hi + 0.05) / (lo + 0.05)


class Document(HTMLParser):
    def __init__(self):
        super().__init__()
        self.tags = []
        self.attrs = []
        self.ids = set()
        self.fragments = []
        self.text = []

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        self.tags.append(tag)
        self.attrs.append((tag, values))
        if "id" in values:
            self.ids.add(values["id"])
        href = values.get("href", "")
        if href.startswith("#"):
            self.fragments.append(href[1:])

    def handle_data(self, data):
        self.text.append(data)


def tokens(source):
    """`--md-<name>: light-dark(<light>, <dark>)` pairs from the token block."""
    found = {}
    for name, light, dark in re.findall(
        r"--md-([\w-]+):\s*light-dark\(\s*(#[0-9a-fA-F]{6})\s*,\s*(#[0-9a-fA-F]{6})\s*\)",
        source,
    ):
        found[name] = (light, dark)
    return found


def check(path):
    source = path.read_text(encoding="utf-8")
    doc = Document()
    doc.feed(source)
    text = " ".join(doc.text)
    problems = []

    def require(condition, message):
        if not condition:
            problems.append(message)

    # --- self-containment: the file has to work with no network at all ------
    external = [
        (tag, v.get("src") or v.get("href"))
        for tag, v in doc.attrs
        if (v.get("src", "") + v.get("href", "")).startswith(("http://", "https://"))
        and tag in ("script", "link", "img", "iframe", "source")
    ]
    require(not external, "loads external resources: %r" % external[:3])
    require(
        not re.search(r"url\(\s*['\"]?https?://", source),
        "references a remote url() in CSS",
    )
    # The math font is the one webfont, and it has to travel inside the file.
    remote_faces = [
        src
        for src in re.findall(r"@font-face\s*{[^}]*?src:\s*url\(([^)]*)\)", source)
        if not src.lstrip("'\"").startswith("data:")
    ]
    require(not remote_faces, "@font-face src is not a data: URI: %r" % remote_faces[:2])
    if "<math" in source:
        require("@font-face" in source, "document has math but ships no math font")
    for tag, v in doc.attrs:
        if tag == "img":
            require(
                (v.get("src") or "").startswith("data:"),
                "img src is not a data: URI: %r" % (v.get("src") or "")[:60],
            )
            require("alt" in v, "img without an alt attribute: %r" % v)

    # --- nothing executable smuggled in from the Markdown -------------------
    for tag, v in doc.attrs:
        for name, value in v.items():
            if name.startswith("on"):
                problems.append("inline event handler %s=%r on <%s>" % (name, value, tag))
    dangerous = [
        v.get("href")
        for tag, v in doc.attrs
        if tag == "a"
        and re.match(r"\s*(javascript|vbscript|data):", v.get("href") or "", re.I)
    ]
    require(not dangerous, "anchors with a scripting scheme: %r" % dangerous[:3])
    require(
        not any(tag == "iframe" for tag, _ in doc.attrs),
        "document contains an <iframe>",
    )

    # --- structure ----------------------------------------------------------
    unresolved = sorted({f for f in doc.fragments if f not in doc.ids})
    require(not unresolved, "in-page links to missing ids: %r" % unresolved[:5])

    # --- contrast, in BOTH themes ------------------------------------------
    # Including the syntax-highlight colours. Checking only the base palette is
    # how a sibling implementation shipped four failing token colours with a
    # green checker.
    pal = tokens(source)
    if "surface" not in pal:
        problems.append("no --md-surface token found; is this an md2pdf document?")
    else:
        for index, theme in enumerate(("light", "dark")):
            bg = pal["surface"][index]
            page = pal["bg"][index] if "bg" in pal else bg
            for name, pair in sorted(pal.items()):
                if name.startswith("t-"):
                    ratio = contrast(pair[index], bg)
                    require(
                        ratio >= AA,
                        "--md-%s %s: %s on %s = %.2f:1 (needs %.1f)"
                        % (name, theme, pair[index], bg, ratio, AA),
                    )
            for name in ("fg", "muted"):
                if name in pal:
                    ratio = contrast(pal[name][index], page)
                    require(
                        ratio >= AA,
                        "--md-%s %s: %s on %s = %.2f:1 (needs %.1f)"
                        % (name, theme, pal[name][index], page, ratio, AA),
                    )

    if problems:
        print("FAIL %s" % path)
        for p in problems:
            print("  - %s" % p)
        return False
    print(
        "ok   %s (%d imgs, %d ids, %d tokens, %d chars of text)"
        % (path, doc.tags.count("img"), len(doc.ids), len(pal), len(text))
    )
    return True


def main(argv):
    if not argv:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    ok = True
    for name in argv:
        path = Path(name)
        if not path.is_file():
            print("FAIL %s: no such file" % path)
            ok = False
            continue
        ok = check(path) and ok
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
