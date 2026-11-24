//! The YAML frontmatter block, read once by the engine for every consumer:
//! the HTML renderer, the image discovery and, through `frontmatter()`, the
//! Typst package.
//!
//! A leading `---` opens a block only when what follows up to the next `---`
//! or `...` line parses as a YAML mapping. Anything else — a thematic break
//! followed by prose, a setext heading — stays part of the document.
//!
//! Scalars keep the text the author wrote. Only `true`/`false` and the null
//! spellings get a type, so `title: 123` or `date: 2024-01-15` reach every
//! renderer as the string they look like.

use saphyr_parser::{Event, Parser, ScalarStyle};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

impl Value {
    /// The value as display text: a scalar as written, a list joined by `, `.
    pub(crate) fn text(&self) -> Option<String> {
        match self {
            Value::Null | Value::Map(_) => None,
            Value::Bool(b) => Some(b.to_string()),
            Value::Str(s) => Some(s.clone()),
            Value::List(items) => {
                let parts: Vec<String> = items.iter().filter_map(Value::text).collect();
                Some(parts.join(", "))
            }
        }
    }

    /// The value as a list of display strings; a scalar is a list of one.
    pub(crate) fn list(&self) -> Vec<String> {
        match self {
            Value::List(items) => items.iter().filter_map(Value::text).collect(),
            other => other.text().into_iter().collect(),
        }
    }

    fn json(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Str(s) => json_string(s, out),
            Value::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.json(out);
                }
                out.push(']');
            }
            Value::Map(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    json_string(key, out);
                    out.push(':');
                    value.json(out);
                }
                out.push('}');
            }
        }
    }
}

fn json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// An explicit no: `false`, `no` or `off`, which YAML 1.2 would otherwise
/// leave as a string.
pub(crate) fn is_off(value: &Value) -> bool {
    match value {
        Value::Bool(b) => !b,
        Value::Str(s) => matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "false" | "no" | "off"
        ),
        _ => false,
    }
}

/// A parsed frontmatter block. Top-level keys are normalised: lowercased, with
/// `_` read as `-`, so `Page_Numbers` and `page-numbers` are one key.
#[derive(Default)]
pub(crate) struct Frontmatter {
    entries: Vec<(String, Value)>,
    /// How many source lines the block takes, delimiters included.
    pub(crate) lines: usize,
}

impl Frontmatter {
    /// The block at the start of `src`, or an empty one when there is none.
    pub(crate) fn parse(src: &str) -> Self {
        let src = src.strip_prefix('\u{feff}').unwrap_or(src);
        let mut lines = src.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
        if lines.next().map(str::trim_end) != Some("---") {
            return Self::default();
        }
        let mut yaml = String::new();
        for (i, line) in lines.enumerate() {
            let t = line.trim_end();
            if t == "---" || t == "..." {
                return match parse_mapping(&yaml) {
                    Some(entries) => Self {
                        entries,
                        lines: i + 2,
                    },
                    None => Self::default(),
                };
            }
            yaml.push_str(line);
            yaml.push('\n');
        }
        Self::default()
    }

    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// A key as display text, `None` when absent or null.
    pub(crate) fn text(&self, key: &str) -> Option<String> {
        self.get(key).and_then(Value::text)
    }

    pub(crate) fn list(&self, key: &str) -> Vec<String> {
        self.get(key).map(Value::list).unwrap_or_default()
    }

    /// A yes/no key. YAML's spellings of true count, the ambiguous ones do not
    /// — a bare `hide-toc-button:` is not a request to hide it.
    pub(crate) fn flag(&self, key: &str) -> bool {
        match self.get(key) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Str(s)) => {
                matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "on" | "1")
            }
            _ => false,
        }
    }

    pub(crate) fn to_json(&self) -> String {
        let mut out = String::new();
        Value::Map(self.entries.clone()).json(&mut out);
        out
    }
}

/// `src` with its frontmatter lines blanked, so the Markdown parsers never see
/// them and every later line keeps its number.
pub(crate) fn blank_frontmatter(src: &str) -> String {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let n = Frontmatter::parse(src).lines;
    if n == 0 {
        return src.to_string();
    }
    let mut parts = src.splitn(n + 1, '\n');
    let mut out = "\n".repeat(n);
    for _ in 0..n {
        parts.next();
    }
    out.push_str(parts.next().unwrap_or(""));
    out
}

/// Parse a YAML document that must be a mapping. Empty (or comment-only) YAML
/// is an empty mapping.
fn parse_mapping(yaml: &str) -> Option<Vec<(String, Value)>> {
    let mut builder = Builder::default();
    for event in Parser::new_from_str(yaml) {
        let (event, _) = event.ok()?;
        builder.push(event)?;
    }
    match builder.root {
        Some(Value::Map(entries)) => Some(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_lowercase().replace('_', "-"), v))
                .fold(Vec::new(), |mut acc: Vec<(String, Value)>, (k, v)| {
                    acc.retain(|(seen, _)| *seen != k);
                    acc.push((k, v));
                    acc
                }),
        ),
        Some(Value::Null) | None => Some(Vec::new()),
        Some(_) => None,
    }
}

enum Open {
    List(usize, Vec<Value>),
    Map(usize, Vec<(String, Value)>, Option<String>),
}

#[derive(Default)]
struct Builder {
    stack: Vec<Open>,
    anchors: HashMap<usize, Value>,
    root: Option<Value>,
}

impl Builder {
    fn push(&mut self, event: Event) -> Option<()> {
        match event {
            Event::Scalar(text, style, anchor, _) => {
                let value = scalar(&text, style);
                self.value(value, anchor)
            }
            Event::Alias(id) => {
                let value = self.anchors.get(&id).cloned().unwrap_or(Value::Null);
                self.value(value, 0)
            }
            Event::SequenceStart(anchor, _) => {
                self.stack.push(Open::List(anchor, Vec::new()));
                Some(())
            }
            Event::MappingStart(anchor, _) => {
                self.stack.push(Open::Map(anchor, Vec::new(), None));
                Some(())
            }
            Event::SequenceEnd | Event::MappingEnd => {
                let (anchor, value) = match self.stack.pop()? {
                    Open::List(a, items) => (a, Value::List(items)),
                    Open::Map(a, entries, _) => (a, Value::Map(entries)),
                };
                self.value(value, anchor)
            }
            _ => Some(()),
        }
    }

    fn value(&mut self, value: Value, anchor: usize) -> Option<()> {
        if anchor != 0 {
            self.anchors.insert(anchor, value.clone());
        }
        match self.stack.last_mut() {
            None => self.root = Some(value),
            Some(Open::List(_, items)) => items.push(value),
            Some(Open::Map(_, entries, key)) => match key.take() {
                Some(k) => entries.push((k, value)),
                // A key that is not a scalar has no name to look it up by.
                None => *key = Some(value.text().unwrap_or_default()),
            },
        }
        Some(())
    }
}

fn scalar(text: &str, style: ScalarStyle) -> Value {
    if style != ScalarStyle::Plain {
        return Value::Str(text.to_string());
    }
    match text {
        "" | "~" | "null" | "Null" | "NULL" => Value::Null,
        "true" | "True" | "TRUE" => Value::Bool(true),
        "false" | "False" | "FALSE" => Value::Bool(false),
        _ => Value::Str(text.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(src: &str) -> Frontmatter {
        Frontmatter::parse(src)
    }

    #[test]
    fn scalars_keep_the_text_the_author_wrote() {
        let f = fm("---\ntitle: 123\ndate: 2024-01-15\nprice: 1.50\nlang: de_DE\n---\n");
        assert_eq!(f.text("title").as_deref(), Some("123"));
        assert_eq!(f.text("date").as_deref(), Some("2024-01-15"));
        assert_eq!(f.text("price").as_deref(), Some("1.50"));
        assert_eq!(f.lines, 6);
    }

    #[test]
    fn keys_are_normalised() {
        let f = fm("---\nPage_Numbers: false\nHeader_Left: x\n---\n");
        assert_eq!(f.get("page-numbers"), Some(&Value::Bool(false)));
        assert_eq!(f.text("header-left").as_deref(), Some("x"));
    }

    #[test]
    fn block_scalars_flow_lists_and_quotes_are_real_yaml() {
        let f = fm("---\ntitle: >-\n  folded\n  title\nauthor: [\"a, b\", 'it''s']\nnote: |\n  one\n  two\n---\n");
        assert_eq!(f.text("title").as_deref(), Some("folded title"));
        assert_eq!(f.list("author"), vec!["a, b", "it's"]);
        assert_eq!(f.text("note").as_deref(), Some("one\ntwo\n"));
    }

    #[test]
    fn the_block_may_close_with_dots_and_tolerates_bom_and_crlf() {
        assert_eq!(
            fm("---\ntitle: X\n...\nbody").text("title").as_deref(),
            Some("X")
        );
        assert_eq!(
            fm("\u{feff}---\r\ntitle: X\r\n---\r\n")
                .text("title")
                .as_deref(),
            Some("X")
        );
    }

    #[test]
    fn only_a_mapping_is_frontmatter() {
        for src in [
            "---\n\nSome intro text.\n\n---\n\n## After",
            "---\n- a\n- b\n---\nbody",
            "---\nnot: [valid\n---\n",
            "---\ntitle: X\n\n# Body",
        ] {
            assert_eq!(fm(src).lines, 0, "{src}");
            assert_eq!(blank_frontmatter(src), src);
        }
        assert_eq!(fm("---\n---\ntext").lines, 2);
        assert_eq!(fm("---\n# just a comment\n---\n").lines, 3);
    }

    #[test]
    fn blanking_keeps_line_numbers() {
        assert_eq!(blank_frontmatter("---\na: 1\n---\nbody\n"), "\n\n\nbody\n");
    }

    #[test]
    fn json_is_escaped_and_typed() {
        let f = fm("---\ntitle: \"a \\\"q\\\" \\\\ b\"\ncover: false\nx: ~\nlist: [1, two]\n---\n");
        assert_eq!(
            f.to_json(),
            r#"{"title":"a \"q\" \\ b","cover":false,"x":null,"list":["1","two"]}"#
        );
    }

    #[test]
    fn yes_no_spellings() {
        let f = fm("---\na: no\nb: off\nc: false\nd: yes\ne: 'false'\n---\n");
        assert!(!f.flag("a") && !f.flag("b") && !f.flag("c") && !f.flag("e"));
        assert!(f.flag("d"));
    }
}
