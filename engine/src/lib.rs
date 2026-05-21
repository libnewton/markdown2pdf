//! md2pdf engine — Markdown -> Typst markup, as a Typst WASM plugin.
//!
//! Parsing is done by `comrak` (CommonMark + GFM). Custom HackMD-flavoured
//! syntax that comrak does not know (`:::` admonitions, `+++++` spoilers,
//! `==mark==`) is handled by a pre-parse pass and post-parse text scanning.
//! This module is the Rust port of the former `src/pipeline/*` TypeScript.

use comrak::nodes::{AstNode, ListType, NodeValue, TableAlignment};
use comrak::{parse_document, Arena, Options};
use std::collections::{HashMap, HashSet};
use unic_emoji_char::is_emoji;
use wasm_minimal_protocol::*;

initiate_protocol!();

/// Token marking a preserved run of 3+ blank lines.
const EXTRA_BLANK_LINE_TOKEN: &str = "[[md2pdf-blank-line]]";

/// Convert Markdown (UTF-8 bytes) to Typst markup (UTF-8 bytes).
/// `strip_h1` non-empty => drop a leading level-1 heading (it became the title).
#[wasm_func]
pub fn convert(markdown: &[u8], strip_h1: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    Ok(convert_str(src, !strip_h1.is_empty()).into_bytes())
}

/// Plain text of a leading level-1 heading, or empty — used as the title.
#[wasm_func]
pub fn leading_h1(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    let pre = preprocess(src);
    let arena = Arena::new();
    let root = parse_document(&arena, &pre.markdown, &build_options());
    let children: Vec<&AstNode> = root.children().collect();
    let text = match leading_h1_index(&children) {
        Some(i) => plain_text(children[i]).trim().to_string(),
        None => String::new(),
    };
    Ok(text.into_bytes())
}

/// List the remote (http/https) image URLs the document references, one
/// `url<TAB>alias` pair per line. The host shim prefetches these before the
/// real compile — Typst's sandbox cannot fetch them itself.
#[wasm_func]
pub fn remotes(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for (url, alias) in collect_remote_images(src) {
        out.push_str(&url);
        out.push('\t');
        out.push_str(&alias);
        out.push('\n');
    }
    Ok(out.into_bytes())
}

/// List the Twemoji codepoints the document references (unicode emoji +
/// `:shortcodes:`), one per line. The host/worker fetches the matching SVGs.
#[wasm_func]
pub fn twemojis(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    Ok(collect_twemoji_codepoints(src).join("\n").into_bytes())
}

/// Full Markdown -> Typst markup pipeline. Recursive: admonition and spoiler
/// bodies are re-run through it so nested custom syntax works.
fn convert_str(src: &str, strip_h1: bool) -> String {
    let pre = preprocess(src);
    let arena = Arena::new();
    let root = parse_document(&arena, &pre.markdown, &build_options());
    let mut ctx = Ctx {
        footnotes: HashMap::new(),
        admonitions: pre.admonitions,
        spoilers: pre.spoilers,
        table_widths: pre.table_widths,
        pending_widths: std::cell::Cell::new(None),
    };
    ctx.collect_footnotes(root);
    let children: Vec<&AstNode> = root.children().collect();
    let skip = if strip_h1 {
        leading_h1_index(&children)
    } else {
        None
    };
    children
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != skip)
        .map(|(_, c)| ctx.render_block(c, 0))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Index of a leading level-1 heading among root children (skips frontmatter).
fn leading_h1_index(children: &[&AstNode]) -> Option<usize> {
    for (i, c) in children.iter().enumerate() {
        match &c.data.borrow().value {
            NodeValue::FrontMatter(_) => continue,
            NodeValue::Heading(h) if h.level == 1 => return Some(i),
            _ => return None,
        }
    }
    None
}

fn build_options() -> Options<'static> {
    let mut o = Options::default();
    let e = &mut o.extension;
    e.strikethrough = true;
    e.table = true;
    e.tasklist = true;
    e.superscript = true;
    e.subscript = true;
    e.underline = true;
    e.footnotes = true;
    e.autolink = true;
    e.math_dollars = true;
    e.math_code = true;
    // `:smile:` -> unicode emoji; the bundled NotoColorEmoji font renders it.
    e.shortcodes = true;
    // Frontmatter is passed through as a FrontMatter node and skipped here;
    // the Typst package parses it separately.
    e.front_matter_delimiter = Some("---".to_string());
    o
}

// ==========================================================================
// Pre-parse pass — extract custom block syntax comrak cannot parse
// ==========================================================================

struct Admonition {
    kind: String,
    title: String,
    source: String,
}

struct Spoiler {
    summary: String,
    source: String,
}

struct Preprocessed {
    markdown: String,
    admonitions: Vec<Admonition>,
    spoilers: Vec<Spoiler>,
    /// Per-table column-width multipliers, indexed by `<!--tablewidths:N-->`.
    table_widths: Vec<Vec<usize>>,
}

const ADMONITION_KINDS: &[&str] = &[
    "success", "warning", "tip", "info", "danger", "note", // styled callouts
    "left", "center", "right", "row", // layout directives
];

/// Extract `:::kind` and `+++++` blocks, replacing each with an HTML-comment
/// placeholder that comrak parses as a standalone `HtmlBlock`.
fn preprocess(src: &str) -> Preprocessed {
    let normalized = src.replace("\r\n", "\n");
    let md0 = preprocess_blank_lines(&normalized);
    let (md1, admonitions) = preprocess_admonitions(&md0);
    let (md2, spoilers) = preprocess_spoilers(&md1);
    let (md3, table_widths) = preprocess_table_widths(&md2);
    Preprocessed {
        markdown: md3,
        admonitions,
        spoilers,
        table_widths,
    }
}

/// Collapse a run of 3+ blank lines into a `[[md2pdf-blank-line]]` token so the
/// extra vertical space survives parsing. Skips fenced code; only fires between
/// two "preservable" lines (not list/quote/table/rule/fence/pagebreak).
fn preprocess_blank_lines(src: &str) -> String {
    if !src.contains("\n\n\n") {
        return src.to_string();
    }
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some((fc, fl)) = fence_marker(line) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
            out.push(line.to_string());
            i += 1;
            continue;
        }
        if fence.is_some() {
            out.push(line.to_string());
            i += 1;
            continue;
        }
        if line.trim().is_empty() {
            let start = i;
            while i < lines.len() && lines[i].trim().is_empty() {
                i += 1;
            }
            let blank_count = i - start;
            let prev = out.iter().rev().find(|l| !l.trim().is_empty());
            let next = lines.get(i);
            if blank_count >= 2
                && prev.is_some_and(|l| should_preserve_blank(l))
                && next.is_some_and(|l| should_preserve_blank(l))
            {
                out.push(String::new());
                out.push(EXTRA_BLANK_LINE_TOKEN.to_string());
                out.push(String::new());
            } else {
                for _ in 0..blank_count {
                    out.push(String::new());
                }
            }
            continue;
        }
        out.push(line.to_string());
        i += 1;
    }
    out.join("\n")
}

/// Whether a blank gap next to this line should be preserved as vertical space.
fn should_preserve_blank(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.starts_with("```") || t.starts_with("~~~") {
        return false;
    }
    if t.starts_with('>') || t.starts_with('|') || t.contains("[[pagebreak]]") {
        return false;
    }
    if is_thematic_break(t) {
        return false;
    }
    let b = t.as_bytes();
    // Unordered list item: `-`, `*`, `+` followed by space (or end of line).
    if matches!(b[0], b'-' | b'*' | b'+') && (b.len() == 1 || b[1] == b' ') {
        return false;
    }
    // Ordered list item: digits then `.`.
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && t[digits..].starts_with('.') {
        return false;
    }
    true
}

/// A thematic break: 3+ of a single `-`/`*`/`_`, only whitespace otherwise.
fn is_thematic_break(t: &str) -> bool {
    ['-', '*', '_'].iter().any(|&ch| {
        t.chars().filter(|&c| c == ch).count() >= 3
            && t.chars().all(|c| c == ch || c.is_whitespace())
    })
}

/// Match an opening/closing code fence (up to 3 leading spaces): (char, length).
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let first = rest.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let run = rest.chars().take_while(|&c| c == first).count();
    (run >= 3).then_some((first, run))
}

/// Non-standard table column widths: `+`s appended to a GFM separator cell
/// widen that column (`---` = 1fr, `---+` = 2fr, `---++` = 3fr). comrak rejects
/// `+` in delimiter rows, so the `+`s are stripped here and the widths recorded
/// behind a `<!--tablewidths:N-->` placeholder before the header row.
fn preprocess_table_widths(src: &str) -> (String, Vec<Vec<usize>>) {
    let mut blocks: Vec<Vec<usize>> = Vec::new();
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for i in 0..lines.len() {
        let line = lines[i];
        if let Some((fc, fl)) = fence_marker(line) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
            out.push(line.to_string());
            continue;
        }
        if fence.is_some() {
            out.push(line.to_string());
            continue;
        }
        let prev = i.checked_sub(1).map(|p| lines[p]);
        if let Some((widths, stripped)) = parse_separator_widths(line, prev) {
            let id = blocks.len();
            blocks.push(widths);
            let header = out.pop().unwrap_or_default();
            out.push(String::new());
            out.push(format!("<!--tablewidths:{id}-->"));
            out.push(String::new());
            out.push(header);
            out.push(stripped);
            continue;
        }
        out.push(line.to_string());
    }
    (out.join("\n"), blocks)
}

/// If `line` is a GFM separator row carrying `+` width markers (and `prev` is
/// its header row), return the column widths and the `+`-stripped separator.
fn parse_separator_widths(line: &str, prev: Option<&str>) -> Option<(Vec<usize>, String)> {
    if !is_pipe_row(line) {
        return None;
    }
    let prev = prev?;
    if !is_pipe_row(prev) || split_cells(prev).iter().all(|c| is_sep_cell(c)) {
        return None;
    }
    let cells = split_cells(line);
    if cells.is_empty() || !cells.iter().all(|c| is_sep_cell(c)) {
        return None;
    }
    if !cells.iter().any(|c| c.contains('+')) {
        return None;
    }
    let widths = cells.iter().map(|c| 1 + c.matches('+').count()).collect();
    let stripped: Vec<String> = cells.iter().map(|c| c.replace('+', "")).collect();
    Some((widths, rebuild_row(line, &stripped)))
}

fn is_pipe_row(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 2 && t.starts_with('|') && t.ends_with('|')
}

/// A GFM separator cell, optionally with trailing `+` width markers:
/// `^\s*:?-{2,}\+*:?\s*$`.
fn is_sep_cell(cell: &str) -> bool {
    let t = cell.trim();
    let t = t.strip_prefix(':').unwrap_or(t);
    let dashes = t.chars().take_while(|&c| c == '-').count();
    if dashes < 2 {
        return false;
    }
    let rest = t[dashes..].strip_suffix(':').unwrap_or(&t[dashes..]);
    rest.chars().all(|c| c == '+')
}

fn split_cells(line: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = line.split('|').collect();
    if parts.len() >= 2 && parts[0].trim().is_empty() {
        parts.remove(0);
    }
    if parts.last().is_some_and(|p| p.trim().is_empty()) {
        parts.pop();
    }
    parts
}

fn rebuild_row(original: &str, cells: &[String]) -> String {
    let leading = &original[..original.len() - original.trim_start().len()];
    let trailing = &original[original.trim_end().len()..];
    let trimmed = original.trim();
    let left = if trimmed.starts_with('|') { "|" } else { "" };
    let right = if trimmed.ends_with('|') { "|" } else { "" };
    format!("{leading}{left}{}{right}{trailing}", cells.join("|"))
}

/// `:::kind ... :::` — CommonMark fence style. A fence of N colons closes only
/// on a line of N or more colons, so longer fences may nest shorter ones.
fn preprocess_admonitions(src: &str) -> (String, Vec<Admonition>) {
    let mut blocks: Vec<Admonition> = Vec::new();
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    let mut fence: Option<(char, usize)> = None;
    while i < lines.len() {
        // `:::` inside a code fence is literal, not an admonition.
        if let Some((fc, fl)) = fence_marker(lines[i]) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        if let Some((fence_len, kind, title)) =
            fence.is_none().then(|| parse_admonition_open(lines[i])).flatten()
        {
            let mut body: Vec<&str> = Vec::new();
            i += 1;
            while i < lines.len() && !is_colon_closer(lines[i], fence_len) {
                body.push(lines[i]);
                i += 1;
            }
            i += 1; // skip closing fence
            let id = blocks.len();
            blocks.push(Admonition {
                kind,
                title,
                source: body.join("\n"),
            });
            out.push(String::new());
            out.push(format!("<!--admonition:{id}-->"));
            out.push(String::new());
            continue;
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    (out.join("\n"), blocks)
}

/// `+++++ ... +++++` — first non-blank inner line (or trailing text on the
/// opener) is the summary; the rest is the spoiler body.
fn preprocess_spoilers(src: &str) -> (String, Vec<Spoiler>) {
    let mut blocks: Vec<Spoiler> = Vec::new();
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    let mut fence: Option<(char, usize)> = None;
    while i < lines.len() {
        // `+++++` inside a code fence is literal, not a spoiler.
        if let Some((fc, fl)) = fence_marker(lines[i]) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        if let Some(inline) =
            fence.is_none().then(|| parse_spoiler_open(lines[i])).flatten()
        {
            let close = ((i + 1)..lines.len()).find(|&j| is_spoiler_closer(lines[j]));
            if let Some(close) = close {
                let mut body: Vec<&str> = lines[(i + 1)..close].to_vec();
                let summary = if !inline.is_empty() {
                    inline
                } else {
                    let mut k = 0;
                    while k < body.len() && body[k].trim().is_empty() {
                        k += 1;
                    }
                    if k < body.len() {
                        let s = body[k].trim().to_string();
                        body = body[(k + 1)..].to_vec();
                        s
                    } else {
                        String::new()
                    }
                };
                let id = blocks.len();
                blocks.push(Spoiler {
                    summary: if summary.is_empty() {
                        "spoiler".to_string()
                    } else {
                        summary
                    },
                    source: body.join("\n"),
                });
                out.push(String::new());
                out.push(format!("<!--spoiler:{id}-->"));
                out.push(String::new());
                i = close + 1;
                continue;
            }
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    (out.join("\n"), blocks)
}

/// Parse a `:::kind title` opener -> (fence length, kind, title).
fn parse_admonition_open(line: &str) -> Option<(usize, String, String)> {
    let bytes = line.as_bytes();
    let fence_len = bytes.iter().take_while(|&&b| b == b':').count();
    if fence_len < 3 {
        return None;
    }
    let rest = line[fence_len..].trim_start();
    let first = rest.chars().next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let mut end = first.len_utf8();
    for (i, c) in rest.char_indices().skip(1) {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    let kind = rest[..end].to_ascii_lowercase();
    if !ADMONITION_KINDS.contains(&kind.as_str()) {
        return None;
    }
    Some((fence_len, kind, rest[end..].trim().to_string()))
}

/// A line of `fence_len`+ colons and nothing else closes an admonition.
fn is_colon_closer(line: &str, fence_len: usize) -> bool {
    let t = line.trim_end();
    t.len() >= fence_len && !t.is_empty() && t.bytes().all(|b| b == b':')
}

/// Parse a `+++++ summary` opener -> the (possibly empty) trailing summary.
fn parse_spoiler_open(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let spaces = bytes.iter().take_while(|&&b| b == b' ').count();
    if spaces > 3 {
        return None;
    }
    let plus = bytes[spaces..].iter().take_while(|&&b| b == b'+').count();
    if plus < 5 {
        return None;
    }
    Some(line[spaces + plus..].trim().to_string())
}

/// A line of `+++++`+ (up to 3 leading spaces) closes a spoiler.
fn is_spoiler_closer(line: &str) -> bool {
    let bytes = line.as_bytes();
    let spaces = bytes.iter().take_while(|&&b| b == b' ').count();
    if spaces > 3 {
        return false;
    }
    let plus = bytes[spaces..].iter().take_while(|&&b| b == b'+').count();
    plus >= 5 && line[spaces + plus..].trim().is_empty()
}

/// Recover the block id from a `<!--admonition:N-->` / `<!--spoiler:N-->`
/// placeholder comrak parsed as an HtmlBlock.
fn parse_placeholder(literal: &str, kind: &str) -> Option<usize> {
    let inner = literal.trim().strip_prefix("<!--")?.strip_suffix("-->")?;
    inner.strip_prefix(kind)?.strip_prefix(':')?.trim().parse().ok()
}

// ==========================================================================
// Rendering context
// ==========================================================================

struct Ctx<'a> {
    /// Footnote definitions keyed by name, rendered inline at the reference.
    footnotes: HashMap<String, &'a AstNode<'a>>,
    /// `:::kind` blocks extracted before parsing, indexed by placeholder id.
    admonitions: Vec<Admonition>,
    /// `+++++` spoiler blocks extracted before parsing.
    spoilers: Vec<Spoiler>,
    /// Column widths per table, indexed by `<!--tablewidths:N-->` id.
    table_widths: Vec<Vec<usize>>,
    /// Width id set by a `tablewidths` placeholder, consumed by the next table.
    pending_widths: std::cell::Cell<Option<usize>>,
}

impl<'a> Ctx<'a> {
    fn collect_footnotes(&mut self, node: &'a AstNode<'a>) {
        if let NodeValue::FootnoteDefinition(def) = &node.data.borrow().value {
            self.footnotes.insert(def.name.clone(), node);
        }
        for child in node.children() {
            self.collect_footnotes(child);
        }
    }

    // ---- block level --------------------------------------------------------

    fn render_block_children(&self, node: &'a AstNode<'a>) -> String {
        node.children()
            .map(|c| self.render_block(c, 0))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn render_block(&self, node: &'a AstNode<'a>, indent: usize) -> String {
        let value = node.data.borrow().value.clone();
        let out = match value {
            NodeValue::FrontMatter(_) => String::new(),
            NodeValue::Document => self.render_block_children(node),
            NodeValue::Heading(h) => {
                let level = h.level.clamp(1, 6) as usize;
                format!("{} {}", "=".repeat(level), self.render_inlines(node))
            }
            NodeValue::Paragraph => self.render_paragraph(node),
            NodeValue::ThematicBreak => "#line(length: 100%, stroke: 0.6pt)".to_string(),
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => {
                let body = self.render_block_children(node);
                if body.trim().is_empty() {
                    "#quote[]".to_string()
                } else {
                    format!("#quote[\n{}\n]", indent_lines(&body, 1))
                }
            }
            NodeValue::List(_) => return self.render_list(node, indent),
            NodeValue::Item(_) | NodeValue::TaskItem(_) => self.render_block_children(node),
            NodeValue::CodeBlock(cb) => return self.render_code_block(&cb.info, &cb.literal, indent),
            NodeValue::HtmlBlock(hb) => {
                if let Some(id) = parse_placeholder(&hb.literal, "admonition") {
                    self.render_admonition(id)
                } else if let Some(id) = parse_placeholder(&hb.literal, "spoiler") {
                    self.render_spoiler(id)
                } else if let Some(id) = parse_placeholder(&hb.literal, "tablewidths") {
                    self.pending_widths.set(Some(id));
                    String::new()
                } else {
                    esc_text(&hb.literal)
                }
            }
            NodeValue::Table(_) => return self.render_table(node, indent),
            NodeValue::FootnoteDefinition(_) => String::new(),
            NodeValue::Math(m) => render_math(m.display_math, &m.literal),
            // Anything else: fall back to inline rendering.
            _ => self.render_inline(node),
        };
        indent_lines(&out, indent)
    }

    fn render_paragraph(&self, node: &'a AstNode<'a>) -> String {
        let plain = plain_text(node);
        match plain.trim().to_ascii_lowercase().as_str() {
            "[toc]" => return "#outline(title: auto, indent: auto)".to_string(),
            "[[pagebreak]]" => return "#pagebreak()".to_string(),
            "[[md2pdf-blank-line]]" => return "#v(0.5em)".to_string(),
            _ => {}
        }
        self.render_inlines(node)
    }

    fn render_admonition(&self, id: usize) -> String {
        let a = match self.admonitions.get(id) {
            Some(a) => a,
            None => return String::new(),
        };
        match a.kind.as_str() {
            // Layout directives render to plain Typst primitives.
            "left" | "center" | "right" => {
                let inner = convert_str(&a.source, false);
                if inner.trim().is_empty() {
                    String::new()
                } else {
                    format!("#align({})[\n{}\n]", a.kind, indent_lines(&inner, 1))
                }
            }
            "row" => render_row(&a.source),
            // Styled callout box.
            _ => {
                let inner = convert_str(&a.source, false);
                let title = if a.title.is_empty() {
                    String::new()
                } else {
                    format!(", title: \"{}\"", esc_string(&a.title))
                };
                format!(
                    "#admonition(kind: \"{}\"{})[\n{}\n]",
                    a.kind,
                    title,
                    indent_lines(&inner, 1)
                )
            }
        }
    }

    fn render_spoiler(&self, id: usize) -> String {
        let s = match self.spoilers.get(id) {
            Some(s) => s,
            None => return String::new(),
        };
        let inner = convert_str(&s.source, false);
        format!(
            "#spoiler(summary: \"{}\")[\n{}\n]",
            esc_string(&s.summary),
            indent_lines(&inner, 1)
        )
    }

    fn render_code_block(&self, info: &str, literal: &str, indent: usize) -> String {
        let code = literal.strip_suffix('\n').unwrap_or(literal);
        let info = info.trim();
        // `mermaid` fences are rendered as diagrams via the mmdr Typst package.
        if info.eq_ignore_ascii_case("mermaid") {
            return indent_lines(&format!("#md-mermaid(\"{}\")", esc_string(code)), indent);
        }
        let fence = "`".repeat(max_backtick_run(code) + 1);
        let open = if info.is_empty() {
            fence.clone()
        } else {
            format!("{fence}{info}")
        };
        indent_lines(&format!("{open}\n{code}\n{fence}"), indent)
    }

    fn render_list(&self, node: &'a AstNode<'a>, indent: usize) -> String {
        let nl = match &node.data.borrow().value {
            NodeValue::List(nl) => nl.clone(),
            _ => return String::new(),
        };
        if nl.is_task_list {
            return node
                .children()
                .map(|item| self.render_task_item(item, indent))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
        }
        let marker = if nl.list_type == ListType::Ordered {
            "+"
        } else {
            "-"
        };
        node.children()
            .map(|item| self.render_list_item(item, marker, indent))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_list_item(&self, item: &'a AstNode<'a>, marker: &str, indent: usize) -> String {
        let base = "  ".repeat(indent);
        let mut lines: Vec<String> = Vec::new();
        let mut first_done = false;
        for child in item.children() {
            let is_para = matches!(child.data.borrow().value, NodeValue::Paragraph);
            if is_para && !first_done {
                lines.push(format!("{base}{marker} {}", self.render_inlines(child)));
                first_done = true;
            } else if matches!(child.data.borrow().value, NodeValue::List(_)) {
                lines.push(self.render_list(child, indent + 1));
            } else {
                lines.push(self.render_block(child, indent + 1));
            }
        }
        if !first_done {
            lines.insert(0, format!("{base}{marker}"));
        }
        lines
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_task_item(&self, item: &'a AstNode<'a>, indent: usize) -> String {
        let base = "  ".repeat(indent);
        let checked = task_checked(item);
        // comrak may wrap the item body inside a TaskItem node — flatten it.
        let mut kids: Vec<&'a AstNode<'a>> = Vec::new();
        for child in item.children() {
            if matches!(child.data.borrow().value, NodeValue::TaskItem(_)) {
                kids.extend(child.children());
            } else {
                kids.push(child);
            }
        }
        let mut body = String::new();
        let mut extras: Vec<String> = Vec::new();
        let mut first_done = false;
        for child in kids {
            let v = child.data.borrow().value.clone();
            match v {
                NodeValue::Paragraph if !first_done => {
                    body = self.render_inlines(child);
                    first_done = true;
                }
                NodeValue::List(_) => extras.push(self.render_list(child, indent + 1)),
                _ => extras.push(self.render_block(child, indent + 1)),
            }
        }
        let head = format!("{base}#task-item({checked})[{body}]");
        std::iter::once(head)
            .chain(extras.into_iter().filter(|s| !s.is_empty()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_table(&self, node: &'a AstNode<'a>, indent: usize) -> String {
        let widths = self
            .pending_widths
            .take()
            .and_then(|id| self.table_widths.get(id));
        let aligns = match &node.data.borrow().value {
            NodeValue::Table(t) => t.alignments.clone(),
            _ => return String::new(),
        };
        let rows: Vec<&AstNode> = node.children().collect();
        if rows.is_empty() {
            return String::new();
        }
        let col_count = rows[0].children().count().max(1);

        let align_word = |a: &TableAlignment| match a {
            TableAlignment::Right => "right",
            TableAlignment::Center => "center",
            _ => "left",
        };
        let align_args: Vec<String> = (0..col_count)
            .map(|i| align_word(aligns.get(i).unwrap_or(&TableAlignment::None)).to_string())
            .collect();

        let cells_of = |row: &'a AstNode<'a>, header: bool| -> Vec<String> {
            row.children()
                .map(|cell| {
                    let inner = self.render_inlines(cell);
                    if header {
                        format!("[*{inner}*]")
                    } else {
                        format!("[{inner}]")
                    }
                })
                .collect()
        };

        let header_cells = cells_of(rows[0], true);
        let data_cells: Vec<String> = rows[1..]
            .iter()
            .flat_map(|row| cells_of(row, false))
            .collect();

        let columns = (0..col_count)
            .map(|i| format!("{}fr", widths.and_then(|w| w.get(i)).copied().unwrap_or(1)))
            .collect::<Vec<_>>()
            .join(", ");
        let body = format!(
            "#table(\n  columns: ({columns}),\n  align: ({align}),\n  table.header({header}),\n  {data}\n)",
            align = align_args.join(", "),
            header = header_cells.join(", "),
            data = data_cells.join(", "),
        );
        indent_lines(&body, indent)
    }

    // ---- inline level -------------------------------------------------------

    fn render_inlines(&self, node: &'a AstNode<'a>) -> String {
        node.children().map(|c| self.render_inline(c)).collect()
    }

    fn render_inline(&self, node: &'a AstNode<'a>) -> String {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::Text(t) => render_text(&t),
            // Soft breaks (source line wraps) become hard breaks, matching the
            // web app's pipeline — it preserves the author's line wrapping.
            NodeValue::SoftBreak => "\\\n".to_string(),
            NodeValue::LineBreak => "\\\n".to_string(),
            NodeValue::Escaped => self.render_inlines(node),
            NodeValue::Emph => format!("#emph[{}]", self.render_inlines(node)),
            NodeValue::Strong => format!("#strong[{}]", self.render_inlines(node)),
            NodeValue::Strikethrough => format!("#strike[{}]", self.render_inlines(node)),
            NodeValue::Superscript => format!("#super[{}]", self.render_inlines(node)),
            NodeValue::Subscript => format!("#sub[{}]", self.render_inlines(node)),
            NodeValue::Underline => format!("#underline[{}]", self.render_inlines(node)),
            NodeValue::Code(c) => render_inline_code(&c.literal),
            NodeValue::Math(m) => render_math(m.display_math, &m.literal),
            NodeValue::HtmlInline(h) => match h.trim().to_ascii_lowercase().as_str() {
                "<u>" => "#underline[".to_string(),
                "</u>" => "]".to_string(),
                _ => esc_text(&h),
            },
            NodeValue::ShortCode(s) => render_emoji(&s.emoji),
            NodeValue::Link(l) => render_link(&l.url, &self.render_inlines(node)),
            NodeValue::Image(l) => render_image(&l.url, &l.title, &plain_text(node)),
            NodeValue::FootnoteReference(r) => self.render_footnote(&r.name),
            // Block nodes should not appear here, but render defensively.
            _ => self.render_inlines(node),
        }
    }

    fn render_footnote(&self, name: &str) -> String {
        match self.footnotes.get(name) {
            None => String::new(),
            Some(def) => {
                let content = def
                    .children()
                    .map(|c| self.render_block(c, 0))
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("#footnote[{}]", content.trim())
            }
        }
    }
}

/// Render a `:::row` block: each top-level child block becomes a grid column.
fn render_row(source: &str) -> String {
    if source.trim().is_empty() {
        return String::new();
    }
    let pre = preprocess(source);
    let arena = Arena::new();
    let root = parse_document(&arena, &pre.markdown, &build_options());
    let mut ctx = Ctx {
        footnotes: HashMap::new(),
        admonitions: pre.admonitions,
        spoilers: pre.spoilers,
        table_widths: pre.table_widths,
        pending_widths: std::cell::Cell::new(None),
    };
    ctx.collect_footnotes(root);
    let cells: Vec<String> = root
        .children()
        .map(|c| ctx.render_block(c, 0))
        .filter(|s| !s.is_empty())
        .map(|c| format!("[\n{}\n]", indent_lines(&c, 1)))
        .collect();
    if cells.is_empty() {
        return String::new();
    }
    let cols = vec!["1fr"; cells.len()].join(", ");
    format!(
        "#grid(columns: ({cols}), column-gutter: 1em, row-gutter: 1em,\n{}\n)",
        cells.join(",\n")
    )
}

// ==========================================================================
// Leaf renderers
// ==========================================================================

/// Render a text run, turning `==mark==` spans into `#highlight[...]`.
/// Matches the former `remark-mark` plugin: flat text only, no nested markup.
fn render_text(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("==") {
        let after = &rest[start + 2..];
        if let Some(end_rel) = after.find("==") {
            let inner = &after[..end_rel];
            if !inner.is_empty() && !inner.starts_with('=') && !inner.ends_with('=') {
                out.push_str(&emit_text(&rest[..start]));
                out.push_str("#highlight[");
                out.push_str(&emit_text(inner));
                out.push(']');
                rest = &after[end_rel + 2..];
                continue;
            }
        }
        // Not a mark span: emit the first `=` literally and keep scanning.
        out.push_str(&emit_text(&rest[..start + 1]));
        rest = &rest[start + 1..];
    }
    out.push_str(&emit_text(rest));
    out
}

/// Escape text for Typst markup, replacing emoji with `#twemoji("cp")` calls.
fn emit_text(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some((end, cp)) = match_emoji(&chars, i) {
            if !plain.is_empty() {
                out.push_str(&esc_text(&plain));
                plain.clear();
            }
            out.push_str(&format!("#twemoji(\"{cp}\")"));
            i = end;
        } else {
            plain.push(chars[i]);
            i += 1;
        }
    }
    if !plain.is_empty() {
        out.push_str(&esc_text(&plain));
    }
    out
}

/// `#twemoji("cp")` for an already-resolved emoji string (a `:shortcode:`).
fn render_emoji(emoji: &str) -> String {
    format!(
        "#twemoji(\"{}\")",
        twemoji_cp(&emoji.chars().collect::<Vec<_>>())
    )
}

/// Match an emoji sequence at `chars[i]`: an Extended_Pictographic char with
/// optional ZWJ-joined parts and a trailing FE0F, or a flag (two regional
/// indicators). Returns (end index, Twemoji codepoint).
fn match_emoji(chars: &[char], i: usize) -> Option<(usize, String)> {
    let c = chars[i];
    if is_regional(c) {
        if i + 1 < chars.len() && is_regional(chars[i + 1]) {
            return Some((i + 2, twemoji_cp(&chars[i..i + 2])));
        }
        return None;
    }
    if !is_pictographic(c) {
        return None;
    }
    let mut j = i + 1;
    while j + 1 < chars.len() && chars[j] == '\u{200D}' && is_pictographic(chars[j + 1]) {
        j += 2;
    }
    if j < chars.len() && chars[j] == '\u{FE0F}' {
        j += 1;
    }
    Some((j, twemoji_cp(&chars[i..j])))
}

/// Twemoji filename codepoint: lowercase hex codepoints joined by `-`, with
/// U+FE0F variation selectors stripped.
fn twemoji_cp(seq: &[char]) -> String {
    seq.iter()
        .filter(|&&c| c != '\u{FE0F}')
        .map(|c| format!("{:x}", *c as u32))
        .collect::<Vec<_>>()
        .join("-")
}

fn is_regional(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

/// A pictographic emoji char. `is_emoji` also covers ASCII `0-9 # *` (keycap
/// bases) — exclude ASCII so plain digits are not turned into Twemoji.
fn is_pictographic(c: char) -> bool {
    !c.is_ascii() && is_emoji(c)
}

/// Collect every Twemoji codepoint the raw Markdown references — unicode
/// emoji sequences plus `:shortcode:` emoji.
fn collect_twemoji_codepoints(src: &str) -> Vec<String> {
    let mut set: HashSet<String> = HashSet::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if let Some((end, cp)) = match_emoji(&chars, i) {
            set.insert(cp);
            i = end;
        } else {
            i += 1;
        }
    }
    // `:shortcode:` emoji.
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' {
            if let Some(rel) = src[i + 1..].find(':') {
                let word = &src[i + 1..i + 1 + rel];
                let valid = !word.is_empty()
                    && word
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'+' | b'-'));
                if valid {
                    if let Some(e) = emojis::get_by_shortcode(word) {
                        set.insert(twemoji_cp(&e.as_str().chars().collect::<Vec<_>>()));
                        i += 1 + rel + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    v
}

fn render_inline_code(literal: &str) -> String {
    format!("`{}`", literal.replace('`', "\\`"))
}

/// Math is delegated to the Typst package's `md-math` helper (mitex-backed),
/// so the engine carries no LaTeX->Typst conversion.
fn render_math(display: bool, latex: &str) -> String {
    format!("#md-math({display}, \"{}\")", esc_string(latex.trim()))
}

fn render_link(url: &str, label: &str) -> String {
    let label = if label.trim().is_empty() {
        esc_text(url)
    } else {
        label.to_string()
    };
    format!("#link(\"{}\")[{}]", esc_string(url), label)
}

/// Port of `renderImage`: HackMD `=WxH` dimension syntax + remote-URL aliasing.
fn render_image(url: &str, title: &str, alt: &str) -> String {
    let mut url = url.to_string();
    let mut dims = parse_dims(title);
    if dims.is_none() {
        // `![alt](path =200x200)` — dims trailing the URL field.
        if let Some(sp) = url.rfind(char::is_whitespace) {
            let (head, tail) = url.split_at(sp);
            if let Some(d) = parse_dims(tail.trim()) {
                dims = Some(d);
                url = head.trim().to_string();
            }
        }
    }

    let path = if is_remote(&url) {
        format!("remote/{}", hash_url(&url))
    } else {
        url.clone()
    };

    let mut args = vec![format!("\"{}\"", esc_string(&path))];
    match &dims {
        Some((w, h)) => {
            if let Some(w) = w {
                args.push(format!("width: {w}pt"));
            }
            if let Some(h) = h {
                args.push(format!("height: {h}pt"));
            }
        }
        None => args.push("width: 100%".to_string()),
    }
    let image_call = format!("#image({})", args.join(", "));

    // Alt text becomes a small centered caption, unless it is just a dim spec.
    let caption = alt.trim();
    let caption_is_dims =
        caption.starts_with('=') || caption.chars().next().is_some_and(|c| c.is_ascii_digit());
    if caption.is_empty() || caption_is_dims {
        return image_call;
    }
    format!(
        "#block(width: 100%, breakable: false)[\n  #align(center)[{image_call}]\n  #v(0.3em, weak: true)\n  #align(center, text(size: 0.85em, fill: luma(120), [{}]))\n]",
        esc_text(caption)
    )
}

/// Parse a `=200x200`, `=200x`, or `200x200` dimension spec.
fn parse_dims(raw: &str) -> Option<(Option<String>, Option<String>)> {
    let s = raw.trim().trim_start_matches('=');
    let (w, h) = s.split_once(['x', 'X'])?;
    let valid_num = |n: &str| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit() || c == '.');
    if !valid_num(w) {
        return None;
    }
    let height = if h.is_empty() {
        None
    } else if valid_num(h) {
        Some(h.to_string())
    } else {
        return None;
    };
    Some((Some(w.to_string()), height))
}

fn is_remote(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    u.starts_with("http://") || u.starts_with("https://")
}

/// FNV-1a-style 32-bit hash — must stay byte-identical to the host shim's
/// hash so the prefetched `remote/<hash>` files line up.
fn hash_url(url: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in url.bytes() {
        h ^= b as u32;
        h = h.wrapping_add(
            (h << 1)
                .wrapping_add(h << 4)
                .wrapping_add(h << 7)
                .wrapping_add(h << 8)
                .wrapping_add(h << 24),
        );
    }
    format!("{h:08x}")
}

/// Scan raw Markdown for `![...](http(s)://...)` image URLs. Runs on the
/// unprocessed source, so it also catches images inside admonitions/spoilers.
fn collect_remote_images(src: &str) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'!' && bytes[i + 1] == b'[' {
            if let Some(rel_close) = src[i + 2..].find(']') {
                let after = i + 2 + rel_close + 1;
                if after < bytes.len() && bytes[after] == b'(' {
                    let raw = src[after + 1..].trim_start();
                    let raw = raw.strip_prefix('<').unwrap_or(raw);
                    if is_remote(raw) {
                        let url: String = raw
                            .chars()
                            .take_while(|&c| {
                                !c.is_whitespace()
                                    && !matches!(c, ')' | '>' | '"' | '\'')
                            })
                            .collect();
                        if seen.insert(url.clone()) {
                            let alias = format!("remote/{}", hash_url(&url));
                            found.push((url, alias));
                        }
                    }
                }
            }
        }
        i += 1;
    }
    found
}

// ==========================================================================
// Helpers
// ==========================================================================

/// Concatenate the visible text of a node's subtree (Text + Code only).
fn plain_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    fn walk<'a>(node: &'a AstNode<'a>, out: &mut String) {
        match &node.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            _ => {}
        }
        for c in node.children() {
            walk(c, out);
        }
    }
    walk(node, &mut out);
    out
}

/// Whether a task-list item is checked (looks for any `[x]` marker).
fn task_checked<'a>(item: &'a AstNode<'a>) -> bool {
    fn walk<'a>(node: &'a AstNode<'a>) -> bool {
        if let NodeValue::TaskItem(sym) = &node.data.borrow().value {
            if sym.is_some() {
                return true;
            }
        }
        node.children().any(walk)
    }
    walk(item)
}

fn max_backtick_run(s: &str) -> usize {
    let mut max = 0;
    let mut run = 0;
    for c in s.chars() {
        if c == '`' {
            run += 1;
            max = max.max(run);
        } else {
            run = 0;
        }
    }
    max.max(2) // fence is run + 1, so at least ```
}

/// Escape text for Typst markup body.
fn esc_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '#' | '*' | '_' | '`' | '[' | ']' | '$' | '<' | '>' | '@') {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

/// Escape text for a Typst string literal.
fn esc_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            _ => o.push(c),
        }
    }
    o
}

fn indent_lines(text: &str, indent: usize) -> String {
    if indent == 0 || text.is_empty() {
        return text.to_string();
    }
    let pad = "  ".repeat(indent);
    text.lines()
        .map(|l| format!("{pad}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}
