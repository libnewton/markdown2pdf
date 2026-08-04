#!/usr/bin/env python3
import re
import sys
from html.parser import HTMLParser
from pathlib import Path


def luminance(value):
    channels = [int(value[index:index + 2], 16) / 255 for index in (1, 3, 5)]
    linear = [channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4 for channel in channels]
    return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]


def contrast(first, second):
    high, low = sorted((luminance(first), luminance(second)), reverse=True)
    return (high + 0.05) / (low + 0.05)


class Document(HTMLParser):
    def __init__(self):
        super().__init__()
        self.tags = []
        self.attrs = []
        self.text = []
        self.ids = set()
        self.fragments = []

    def handle_starttag(self, tag, attrs):
        self.tags.append(tag)
        values = dict(attrs)
        self.attrs.append((tag, values))
        if "id" in values:
            self.ids.add(values["id"])
        if values.get("href", "").startswith("#"):
            self.fragments.append(values["href"][1:])

    def handle_data(self, data):
        self.text.append(data)


path = Path(sys.argv[1])
source = path.read_text(encoding="utf-8")
document = Document()
document.feed(source)
attrs = document.attrs
text = " ".join(document.text)

html_attrs = next(values for tag, values in attrs if tag == "html")
assert html_attrs.get("lang") == "en"
assert any(tag == "meta" and values.get("name") == "author" for tag, values in attrs)
assert any(tag == "time" and values.get("datetime") == "2026-08-04" for tag, values in attrs)
assert "HTML Edge Fixture" in text
assert [tag for tag in document.tags if re.fullmatch(r"h[1-6]", tag)] == [
    "h1", "h2", "h3", "h4", "h5", "h6", "h2", "h2", "h2", "h2"
]
assert "nav" in document.tags and document.tags.count("details") >= 2
assert document.tags.count("summary") >= 2
theme_buttons = [values for tag, values in attrs if tag == "button" and values.get("id") == "md-theme-toggle"]
assert theme_buttons == [{
    "id": "md-theme-toggle",
    "class": "md-theme-toggle",
    "type": "button",
    "title": "Use dark theme",
    "aria-label": "Use dark theme",
    "aria-pressed": "false",
}]
toc_controls = [values for tag, values in attrs if tag == "details" and values.get("class") == "md-toc"]
assert len(toc_controls) == 1 and "open" not in toc_controls[0]
assert any(tag == "span" and values.get("class") == "md-toc-icon" for tag, values in attrs)
assert any(tag == "span" and values.get("class") == "md-toc-label" for tag, values in attrs)
assert document.fragments and all(fragment in document.ids for fragment in document.fragments)
assert any(tag == "input" and values.get("type") == "checkbox" for tag, values in attrs)
assert document.tags.count("th") == 5
assert "md-footnotes" in source and "Accessible footnote content retention marker" in text
assert all("alt" in values for tag, values in attrs if tag == "img")
assert any(values.get("alt") == 'Local image "alt" and caption' for tag, values in attrs if tag == "img")
assert "Local image “alt” and caption" in text
assert "Keyboard-accessible spoiler content" in text
assert "First responsive column" in text and "Third responsive column" in text
assert not any(tag == "script" and "src" in values for tag, values in attrs)
assert not any(
    tag == "link" and values.get("rel") == "stylesheet" and "href" in values
    for tag, values in attrs
)
assert all(values["src"].startswith("data:") for tag, values in attrs if tag == "img")
assert not re.search(r"@font-face|url\(['\"]?https?://", source)

themes = re.findall(r':root(?:\[data-theme="dark"\])?\s*\{([^}]+)\}', source)
assert len(themes) >= 2
for block in themes[:2]:
    tokens = dict(re.findall(r'--([\w-]+):\s*(#[0-9a-fA-F]{6})', block))
    assert contrast(tokens["text"], tokens["bg"]) >= 4.5
    assert contrast(tokens["muted"], tokens["bg"]) >= 4.5
    assert contrast(tokens["accent"], tokens["bg"]) >= 4.5
    assert contrast(tokens["accent"], tokens["surface"]) >= 3
    for kind in ("info", "tip", "warning", "danger"):
        assert contrast(tokens[kind], tokens[f"{kind}-bg"]) >= 4.5
print(f"ok: {path} ({document.tags.count('img')} embedded images)")
