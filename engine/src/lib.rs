//! md2pdf engine — Markdown -> Typst markup, as a Typst WASM plugin.
//!
//! Parsing is done by `comrak` (CommonMark + GFM). Custom HackMD-flavoured
//! syntax that comrak does not know (`:::` admonitions, `+++++` spoilers,
//! `==mark==`) is handled by a pre-parse pass and post-parse text scanning.
//! This module is the Rust port of the former `src/pipeline/*` TypeScript.

mod html;

use comrak::nodes::{AstNode, ListType, NodeValue, TableAlignment};
use comrak::{parse_document, Arena, Options};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use unic_emoji_char::is_emoji;
use wasm_minimal_protocol::*;

initiate_protocol!();

/// Token marking a preserved run of 3+ blank lines.
const EXTRA_BLANK_LINE_TOKEN: &str = "[[md2pdf-blank-line]]";
const CITATION_OPEN_TOKEN: &str = "\u{e000}md2pdf-cite:";
const CITATION_CLOSE_TOKEN: &str = "\u{e001}";

/// Convert Markdown (UTF-8 bytes) to Typst markup (UTF-8 bytes).
/// `strip_h1` non-empty drops a leading level-1 heading; `citations` non-empty
/// enables `[@key]` citation rendering for an extracted inline bibliography.
#[wasm_func]
pub fn convert(markdown: &[u8], strip_h1: &[u8], citations: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    Ok(convert_str_with_citations(src, !strip_h1.is_empty(), !citations.is_empty()).into_bytes())
}

/// Markdown preceding a trailing inline BibTeX block, or the original source.
#[wasm_func]
pub fn without_inline_bibliography(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    Ok(split_inline_bibliography(src).0.into_bytes())
}

/// A trailing inline BibTeX block, or empty when none is present.
#[wasm_func]
pub fn inline_bibliography(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    Ok(split_inline_bibliography(src).1.into_bytes())
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
        Some(i) => heading_parts(&plain_text(children[i])).0,
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

/// The shared design tokens as TOML: callout colours and labels, plus the base
/// palette. The stylesheet bakes the same table in, so the Typst templates and
/// the HTML output cannot drift apart.
#[wasm_func]
pub fn tokens() -> Result<Vec<u8>, String> {
    Ok(html::tokens_toml().into_bytes())
}

/// List the local (non-remote) image paths the document references, one per
/// line — the HTML output embeds them, so the host has to supply the bytes.
#[wasm_func]
pub fn html_images(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    Ok(html::local_images(src).join("\n").into_bytes())
}

/// The font files a standalone HTML export needs, one key per line — empty
/// unless the document has math. MathML laid out without a MATH-table font
/// looks nothing like the PDF, so the export embeds one.
#[wasm_func]
pub fn html_fonts(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    if !has_math(src) {
        return Ok(Vec::new());
    }
    Ok(b"fonts/math.woff2\nfonts/math-alpha.woff2\n".to_vec())
}

/// List the Mermaid diagram sources, one `key<TAB>source` pair per line with
/// newlines in the source escaped as `\n`. The host renders each to SVG and
/// returns it under `key`.
#[wasm_func]
pub fn html_mermaid(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for code in collect_mermaid_sources(src) {
        out.push_str(&html::mermaid_key(&code));
        out.push('\t');
        out.push_str(&code.replace('\\', "\\\\").replace('\n', "\\n"));
        out.push('\n');
    }
    Ok(out.into_bytes())
}

/// Everything an HTML render needs the host to fetch, from one parse.
///
/// One `kind<TAB>value…` line per asset, where `kind` is `image`, `remote`
/// (`url<TAB>alias`), `emoji`, `font` or `mermaid` (`key<TAB>escaped-source`).
/// The individual `html_images` / `remotes` / `twemojis` / `html_fonts` /
/// `html_mermaid` calls each re-parse the document; asking for all five at
/// once is the difference between five parses per keystroke and one.
#[wasm_func]
pub fn html_assets(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for path in html::local_images(src) {
        out.push_str("image\t");
        out.push_str(&path);
        out.push('\n');
    }
    for (url, alias) in collect_remote_images(src) {
        out.push_str("remote\t");
        out.push_str(&url);
        out.push('\t');
        out.push_str(&alias);
        out.push('\n');
    }
    for cp in collect_twemoji_codepoints(src) {
        out.push_str("emoji\t");
        out.push_str(&cp);
        out.push('\n');
    }
    if has_math(src) {
        out.push_str("font\tfonts/math.woff2\nfont\tfonts/math-alpha.woff2\n");
    }
    for code in collect_mermaid_sources(src) {
        out.push_str("mermaid\t");
        out.push_str(&html::mermaid_key(&code));
        out.push('\t');
        out.push_str(&code.replace('\\', "\\\\").replace('\n', "\\n"));
        out.push('\n');
    }
    Ok(out.into_bytes())
}

/// Render Markdown to HTML.
///
/// `options` is a `key=value` line block (`standalone=1` wraps the fragment in
/// a full document). `manifest` is `key<TAB>byte-length` lines describing how
/// to slice `assets`, the concatenated bytes of every image, Twemoji SVG and
/// rendered Mermaid diagram the host resolved for us.
#[wasm_func]
pub fn render_html(
    markdown: &[u8],
    options: &[u8],
    manifest: &[u8],
    assets: &[u8],
) -> Result<Vec<u8>, String> {
    let utf8 = |b| std::str::from_utf8(b).map_err(|e: std::str::Utf8Error| e.to_string());
    Ok(html::render(utf8(markdown)?, utf8(options)?, utf8(manifest)?, assets).into_bytes())
}

/// Full Markdown -> Typst markup pipeline. Recursive: admonition and spoiler
/// bodies are re-run through it so nested custom syntax works.
#[cfg(test)]
fn convert_str(src: &str, strip_h1: bool) -> String {
    convert_str_with_citations(src, strip_h1, false)
}

fn convert_str_with_citations(src: &str, strip_h1: bool, citations: bool) -> String {
    convert_str_aligned(src, strip_h1, None, citations)
}

fn convert_str_aligned(
    src: &str,
    strip_h1: bool,
    alignment: Option<Alignment>,
    citations: bool,
) -> String {
    convert_str_aligned_with(
        src,
        strip_h1,
        alignment,
        citations,
        Rc::new(std::cell::RefCell::new(HashMap::new())),
    )
}

fn convert_str_aligned_with(
    src: &str,
    strip_h1: bool,
    alignment: Option<Alignment>,
    citations: bool,
    slugs: Rc<std::cell::RefCell<HashMap<String, usize>>>,
) -> String {
    let citation_source = if citations {
        preprocess_citations(src)
    } else {
        src.to_string()
    };
    let pre = preprocess(&citation_source);
    let arena = Arena::new();
    let root = parse_document(&arena, &pre.markdown, &build_options());
    let mut ctx = Ctx {
        footnotes: HashMap::new(),
        admonitions: pre.admonitions,
        spoilers: pre.spoilers,
        table_widths: pre.table_widths,
        pending_widths: std::cell::Cell::new(None),
        rendering_notes: std::cell::RefCell::new(HashSet::new()),
        slugs,
        alignment,
        citations,
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

/// One line of Markdown plus the 1-based line of the source it came from, or
/// 0 for a line a pass invented.
///
/// The passes below rewrite whole blocks — a `:::info` of any length becomes
/// three lines, a run of blank lines becomes three — so comrak's line numbers
/// describe the text it was handed and not the text the author wrote.
/// Carrying the origin along is what lets a rendered element point back at
/// the line that produced it.
type Line = (String, u32);

fn as_lines(src: &str) -> Vec<Line> {
    src.split('\n')
        .enumerate()
        .map(|(i, l)| (l.to_string(), i as u32 + 1))
        .collect()
}

fn join_lines(lines: &[Line]) -> (String, Vec<u32>) {
    let text = lines.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join("\n");
    (text, lines.iter().map(|(_, o)| *o).collect())
}

/// Map origins expressed in `base`'s coordinates into `base`'s own, for the
/// passes that re-run over a block's extracted body.
pub(crate) fn rebase(local: &[u32], base: &[u32]) -> Vec<u32> {
    local
        .iter()
        .map(|&l| if l == 0 { 0 } else { base.get(l as usize - 1).copied().unwrap_or(0) })
        .collect()
}

struct Admonition {
    kind: String,
    title: String,
    source: String,
    origin: Vec<u32>,
}

struct Spoiler {
    summary: String,
    source: String,
    origin: Vec<u32>,
}

struct Preprocessed {
    markdown: String,
    /// One entry per line of `markdown`, naming its line in the original source.
    origin: Vec<u32>,
    admonitions: Vec<Admonition>,
    spoilers: Vec<Spoiler>,
    /// Per-table column-width multipliers, indexed by `<!--tablewidths:N-->`.
    table_widths: Vec<Vec<usize>>,
}

/// Split a trailing inline BibTeX section from Markdown. Entry openers inside
/// fenced code are ignored, so examples remain ordinary document content.
fn split_inline_bibliography(src: &str) -> (String, String) {
    let mut fence: Option<(char, usize)> = None;
    let mut offset = 0;
    for line in src.split_inclusive('\n') {
        let plain = line.strip_suffix('\n').unwrap_or(line);
        if let Some((fc, fl)) = fence_marker(plain) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
        } else if fence.is_none() && is_bibtex_entry_open(plain.trim()) {
            return (
                src[..offset].trim_end().to_string(),
                src[offset..].trim().to_string(),
            );
        }
        offset += line.len();
    }
    (src.to_string(), String::new())
}

fn is_bibtex_entry_open(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('@') else {
        return false;
    };
    let kind_len = rest.bytes().take_while(|b| b.is_ascii_alphabetic()).count();
    kind_len > 0 && rest.as_bytes().get(kind_len) == Some(&b'{')
}

fn preprocess_citations(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut fence: Option<(char, usize)> = None;
    for line in src.split_inclusive('\n') {
        let plain = line.strip_suffix('\n').unwrap_or(line);
        if let Some((fc, fl)) = fence_marker(plain) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
            out.push_str(line);
        } else if fence.is_some() {
            out.push_str(line);
        } else {
            out.push_str(&replace_citations_outside_code_spans(line));
        }
    }
    out
}

fn replace_citations_outside_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    let mut code_ticks = 0;
    while !rest.is_empty() {
        let ticks = rest.bytes().take_while(|b| *b == b'`').count();
        if ticks > 0 {
            if code_ticks == 0 {
                code_ticks = ticks;
            } else if code_ticks == ticks {
                code_ticks = 0;
            }
            out.push_str(&rest[..ticks]);
            rest = &rest[ticks..];
            continue;
        }
        if code_ticks == 0 && rest.starts_with("[@") {
            if let Some(end) = rest.find(']') {
                let group = &rest[1..end];
                if citation_keys(group).is_some() {
                    out.push_str(CITATION_OPEN_TOKEN);
                    out.push_str(group);
                    out.push_str(CITATION_CLOSE_TOKEN);
                    rest = &rest[end + 1..];
                    continue;
                }
            }
        }
        let char_len = rest.chars().next().unwrap().len_utf8();
        out.push_str(&rest[..char_len]);
        rest = &rest[char_len..];
    }
    out
}

fn citation_keys(group: &str) -> Option<Vec<&str>> {
    let mut keys = Vec::new();
    for part in group.split(',') {
        let key = part.trim().strip_prefix('@')?;
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_:.+-".contains(c))
        {
            return None;
        }
        keys.push(key);
    }
    (!keys.is_empty()).then_some(keys)
}

const ADMONITION_KINDS: &[&str] = &[
    "success", "warning", "tip", "info", "danger", "note", "caution", "important",
    "left", "center", "right", "row", // layout directives
];

/// Extract `:::kind` and `+++++` blocks, replacing each with an HTML-comment
/// placeholder that comrak parses as a standalone `HtmlBlock`.
fn preprocess(src: &str) -> Preprocessed {
    let normalized = src.replace("\r\n", "\n");
    let l0 = preprocess_blank_lines(as_lines(&normalized));
    let (l1, admonitions) = preprocess_admonitions(l0);
    let (l2, spoilers) = preprocess_spoilers(l1);
    let (l3, table_widths) = preprocess_table_widths(l2);
    let (markdown, origin) = join_lines(&l3);
    Preprocessed {
        markdown,
        origin,
        admonitions,
        spoilers,
        table_widths,
    }
}

/// Collapse a run of 3+ blank lines into a `[[md2pdf-blank-line]]` token so the
/// extra vertical space survives parsing. Skips fenced code; only fires between
/// two "preservable" lines (not list/quote/table/rule/fence/pagebreak).
fn preprocess_blank_lines(lines: Vec<Line>) -> Vec<Line> {
    if !lines.windows(3).any(|w| w.iter().all(|(l, _)| l.trim().is_empty())) {
        return lines;
    }
    let mut out: Vec<Line> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        if let Some((fc, fl)) = fence_marker(&line.0) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
            out.push(line.clone());
            i += 1;
            continue;
        }
        if fence.is_some() {
            out.push(line.clone());
            i += 1;
            continue;
        }
        if line.0.trim().is_empty() {
            let start = i;
            while i < lines.len() && lines[i].0.trim().is_empty() {
                i += 1;
            }
            let blank_count = i - start;
            let prev = out.iter().rev().find(|(l, _)| !l.trim().is_empty());
            let next = lines.get(i);
            if blank_count >= 2
                && prev.is_some_and(|(l, _)| should_preserve_blank(l))
                && next.is_some_and(|(l, _)| should_preserve_blank(l))
            {
                // Three lines stand in for the whole run; the token belongs to
                // no line of the source, so it carries no origin.
                out.push((String::new(), 0));
                out.push((EXTRA_BLANK_LINE_TOKEN.to_string(), 0));
                out.push((String::new(), 0));
            } else {
                out.extend(lines[start..i].iter().cloned());
            }
            continue;
        }
        out.push(line.clone());
        i += 1;
    }
    out
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
fn preprocess_table_widths(lines: Vec<Line>) -> (Vec<Line>, Vec<Vec<usize>>) {
    if !lines
        .iter()
        .any(|(l, _)| l.contains('+') && l.contains('-') && l.contains('|'))
    {
        return (lines, Vec::new());
    }
    let (text, _) = join_lines(&lines);
    let code = code_block_lines(&text);
    let mut blocks: Vec<Vec<usize>> = Vec::new();
    let mut out: Vec<Line> = Vec::new();
    for i in 0..lines.len() {
        let line = &lines[i];
        if code.contains(&(i + 1)) {
            out.push(line.clone());
            continue;
        }
        let prev = i.checked_sub(1).map(|p| lines[p].0.as_str());
        if let Some((widths, stripped)) = parse_separator_widths(&line.0, prev) {
            let id = blocks.len();
            blocks.push(widths);
            let header = out.pop().unwrap_or_default();
            // The placeholder inherits the header's container prefix so a table
            // inside a list item or a blockquote stays inside it. No blank lines
            // around it: an HTML comment is a block on its own and can interrupt
            // a paragraph, and blank lines would loosen an enclosing list.
            let (prefix, _) = split_prefix(&header.0);
            out.push((format!("{prefix}<!--tablewidths:{id}-->"), 0));
            out.push(header);
            // The separator row keeps its own line: only its `+` markers went.
            out.push((stripped, line.1));
            continue;
        }
        out.push(line.clone());
    }
    (out, blocks)
}

/// 1-based line numbers comrak reads as code, fenced or indented. The width
/// pass leaves those alone: a `+` there is sample text, not a marker. Asking
/// the parser is the only reliable way to tell an indented code block from a
/// table nested in a list item.
fn code_block_lines(src: &str) -> HashSet<usize> {
    let arena = Arena::new();
    let root = parse_document(&arena, src, &build_options());
    let mut set = HashSet::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        if matches!(data.value, NodeValue::CodeBlock(_)) {
            set.extend(data.sourcepos.start.line..=data.sourcepos.end.line);
        }
    }
    set
}

/// If `line` is a GFM separator row carrying `+` width markers (and `prev` is
/// its header row), return the column widths and the `+`-stripped separator.
fn parse_separator_widths(line: &str, prev: Option<&str>) -> Option<(Vec<usize>, String)> {
    let (prefix, body) = split_prefix(line);
    if !is_pipe_row(body) {
        return None;
    }
    let (prev_prefix, prev_body) = split_prefix(prev?);
    // Both rows must sit at the same blockquote depth to belong to one table.
    if quote_depth(prefix) != quote_depth(prev_prefix) {
        return None;
    }
    if !is_pipe_row(prev_body) || split_cells(prev_body).iter().all(|c| is_sep_cell(c)) {
        return None;
    }
    let cells = split_cells(body);
    if cells.is_empty() || !cells.iter().all(|c| is_sep_cell(c)) {
        return None;
    }
    if !cells.iter().any(|c| c.contains('+')) {
        return None;
    }
    let widths = cells.iter().map(|c| 1 + c.matches('+').count()).collect();
    let stripped: Vec<String> = cells.iter().map(|c| c.replace('+', "")).collect();
    Some((widths, format!("{prefix}{}", rebuild_row(body, &stripped))))
}

/// Split off a line's block-container prefix (indentation and `>` markers).
fn split_prefix(line: &str) -> (&str, &str) {
    let end = line
        .find(|c: char| c != ' ' && c != '\t' && c != '>')
        .unwrap_or(line.len());
    line.split_at(end)
}

fn quote_depth(prefix: &str) -> usize {
    prefix.matches('>').count()
}

/// A table row: any line with a `|` in it. GFM allows the outer pipes to be
/// omitted, so a leading/trailing `|` cannot be required.
fn is_pipe_row(body: &str) -> bool {
    body.contains('|')
}

/// A GFM separator cell, optionally with `+` width markers on either side of
/// the alignment colon: `^\s*:?-+(\+*:?|:?\+*)\s*$`.
fn is_sep_cell(cell: &str) -> bool {
    let t = cell.trim();
    let t = t.strip_prefix(':').unwrap_or(t);
    let dashes = t.chars().take_while(|&c| c == '-').count();
    if dashes == 0 {
        return false;
    }
    let rest = t[dashes..].trim_end_matches('+');
    let rest = rest.strip_suffix(':').unwrap_or(rest);
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
fn preprocess_admonitions(lines: Vec<Line>) -> (Vec<Line>, Vec<Admonition>) {
    let mut blocks: Vec<Admonition> = Vec::new();
    let mut out: Vec<Line> = Vec::new();
    let mut i = 0;
    let mut fence: Option<(char, usize)> = None;
    while i < lines.len() {
        // `:::` inside a code fence is literal, not an admonition.
        if let Some((fc, fl)) = fence_marker(&lines[i].0) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
            out.push(lines[i].clone());
            i += 1;
            continue;
        }
        if let Some((fence_len, kind, title)) =
            fence.is_none().then(|| parse_admonition_open(&lines[i].0)).flatten()
        {
            let mut body: Vec<Line> = Vec::new();
            i += 1;
            while i < lines.len() && !is_colon_closer(&lines[i].0, fence_len) {
                body.push(lines[i].clone());
                i += 1;
            }
            i += 1; // skip closing fence
            let id = blocks.len();
            let (source, origin) = join_lines(&body);
            // The body is re-parsed as its own document, so it keeps its own
            // origins to be rebased against these when it is rendered.
            blocks.push(Admonition { kind, title, source, origin });
            out.push((String::new(), 0));
            out.push((format!("<!--admonition:{id}-->"), 0));
            out.push((String::new(), 0));
            continue;
        }
        out.push(lines[i].clone());
        i += 1;
    }
    (out, blocks)
}

/// `+++++ ... +++++` — first non-blank inner line (or trailing text on the
/// opener) is the summary; the rest is the spoiler body.
fn preprocess_spoilers(lines: Vec<Line>) -> (Vec<Line>, Vec<Spoiler>) {
    let mut blocks: Vec<Spoiler> = Vec::new();
    let mut out: Vec<Line> = Vec::new();
    let mut i = 0;
    let mut fence: Option<(char, usize)> = None;
    while i < lines.len() {
        // `+++++` inside a code fence is literal, not a spoiler.
        if let Some((fc, fl)) = fence_marker(&lines[i].0) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
            out.push(lines[i].clone());
            i += 1;
            continue;
        }
        if let Some(inline) =
            fence.is_none().then(|| parse_spoiler_open(&lines[i].0)).flatten()
        {
            let close = ((i + 1)..lines.len()).find(|&j| is_spoiler_closer(&lines[j].0));
            if let Some(close) = close {
                let mut body: Vec<Line> = lines[(i + 1)..close].to_vec();
                let summary = if !inline.is_empty() {
                    inline
                } else {
                    let mut k = 0;
                    while k < body.len() && body[k].0.trim().is_empty() {
                        k += 1;
                    }
                    if k < body.len() {
                        let s = body[k].0.trim().to_string();
                        body = body[(k + 1)..].to_vec();
                        s
                    } else {
                        String::new()
                    }
                };
                let id = blocks.len();
                let (source, origin) = join_lines(&body);
                blocks.push(Spoiler {
                    summary: if summary.is_empty() {
                        "spoiler".to_string()
                    } else {
                        summary
                    },
                    source,
                    origin,
                });
                out.push((String::new(), 0));
                out.push((format!("<!--spoiler:{id}-->"), 0));
                out.push((String::new(), 0));
                i = close + 1;
                continue;
            }
        }
        out.push(lines[i].clone());
        i += 1;
    }
    (out, blocks)
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

#[derive(Clone, Copy)]
enum Alignment {
    Left,
    Center,
    Right,
}

impl Alignment {
    fn from_kind(kind: &str) -> Option<Self> {
        match kind {
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    fn typst(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

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
    /// Footnotes currently being rendered. A note that references itself would
    /// otherwise recurse until the plugin's stack is exhausted.
    rendering_notes: std::cell::RefCell<HashSet<String>>,
    /// Heading ids already emitted, for stable duplicate suffixes.
    slugs: Rc<std::cell::RefCell<HashMap<String, usize>>>,
    alignment: Option<Alignment>,
    citations: bool,
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
        // A pending width id belongs to the table that directly follows its
        // placeholder. If anything else intervenes the markers never became a
        // table, so drop them rather than let them land on a later one.
        if !matches!(value, NodeValue::Table(_) | NodeValue::HtmlBlock(_)) {
            self.pending_widths.set(None);
        }
        let out = match value {
            NodeValue::FrontMatter(_) => String::new(),
            NodeValue::Document => self.render_block_children(node),
            NodeValue::Heading(h) => {
                let level = h.level.clamp(1, 6) as usize;
                let (text, custom) = heading_parts(&plain_text(node));
                let id = unique_heading_id(&mut self.slugs.borrow_mut(), &text, custom.as_deref());
                format!(
                    "{} {} <{}>",
                    "=".repeat(level),
                    self.render_heading_inlines(node),
                    id
                )
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
            NodeValue::Math(m) => render_math(m.display_math, &m.literal, self.visual_alignment()),
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
                let alignment = Alignment::from_kind(&a.kind).unwrap();
                let inner = convert_str_aligned_with(
                    &a.source,
                    false,
                    Some(alignment),
                    self.citations,
                    self.slugs.clone(),
                );
                if inner.trim().is_empty() {
                    String::new()
                } else {
                    format!(
                        "#align({})[\n{}\n]",
                        alignment.typst(),
                        indent_lines(&inner, 1)
                    )
                }
            }
            "row" => render_row(&a.source, self.alignment, self.citations, self.slugs.clone()),
            // Styled callout box.
            _ => {
                let inner = convert_str_aligned_with(
                    &a.source,
                    false,
                    self.alignment,
                    self.citations,
                    self.slugs.clone(),
                );
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
        let inner = convert_str_aligned_with(
            &s.source,
            false,
            self.alignment,
            self.citations,
            self.slugs.clone(),
        );
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
            return indent_lines(
                &format!(
                    "#align({})[#md-mermaid(\"{}\")]",
                    self.visual_alignment().typst(),
                    esc_string(code)
                ),
                indent,
            );
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

    fn render_heading_inlines(&self, node: &'a AstNode<'a>) -> String {
        let children: Vec<&AstNode> = node.children().collect();
        children
            .iter()
            .enumerate()
            .map(|(i, child)| {
                if i + 1 == children.len() {
                    if let NodeValue::Text(text) = &child.data.borrow().value {
                        let (visible, custom) = heading_parts(text);
                        if custom.is_some() {
                            return render_text(&visible, self.citations);
                        }
                    }
                }
                self.render_inline(child)
            })
            .collect()
    }

    fn render_inline(&self, node: &'a AstNode<'a>) -> String {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::Text(t) => render_text(&t, self.citations),
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
            NodeValue::Math(m) => render_math(m.display_math, &m.literal, self.visual_alignment()),
            NodeValue::HtmlInline(h) => match h.trim().to_ascii_lowercase().as_str() {
                // Table cells hold inline content only, so `<br>` is the one
                // way to get a second line into one. It is also the only raw
                // tag either renderer honours; everything else is text.
                "<br>" | "<br/>" | "<br />" => "#linebreak()".to_string(),
                _ => esc_text(&h),
            },
            NodeValue::ShortCode(s) => render_emoji(&s.emoji),
            NodeValue::Link(l) => render_link(&l.url, &self.render_inlines(node)),
            NodeValue::Image(l) => render_image(
                &l.url,
                &l.title,
                &plain_text(node),
                self.visual_alignment(),
            ),
            NodeValue::FootnoteReference(r) => self.render_footnote(&r.name),
            // Block nodes should not appear here, but render defensively.
            _ => self.render_inlines(node),
        }
    }

    fn render_footnote(&self, name: &str) -> String {
        let Some(def) = self.footnotes.get(name) else {
            return String::new();
        };
        if !self.rendering_notes.borrow_mut().insert(name.to_string()) {
            // Already inside this note: drop the back-reference rather than
            // recurse into it forever.
            return String::new();
        }
        let content = def
            .children()
            .map(|c| self.render_block(c, 0))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        self.rendering_notes.borrow_mut().remove(name);
        format!("#footnote[{}]", content.trim())
    }

    fn visual_alignment(&self) -> Alignment {
        self.alignment.unwrap_or(Alignment::Center)
    }
}

/// Render a `:::row` block: each top-level child block becomes a grid column.
fn render_row(
    source: &str,
    alignment: Option<Alignment>,
    citations: bool,
    slugs: Rc<std::cell::RefCell<HashMap<String, usize>>>,
) -> String {
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
        rendering_notes: std::cell::RefCell::new(HashSet::new()),
        slugs,
        alignment,
        citations,
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
fn render_text(s: &str, citations: bool) -> String {
    let mut out = String::new();
    let mut rest = s;
    loop {
        let mark = rest.find("==");
        let citation = if citations {
            rest.find(CITATION_OPEN_TOKEN)
        } else {
            None
        };
        let next = match (mark, citation) {
            (None, None) => break,
            (Some(m), None) => (m, false),
            (None, Some(c)) => (c, true),
            (Some(m), Some(c)) if c < m => (c, true),
            (Some(m), Some(_)) => (m, false),
        };

        if next.1 {
            let after = &rest[next.0 + CITATION_OPEN_TOKEN.len()..];
            if let Some(end) = after.find(CITATION_CLOSE_TOKEN) {
                if let Some(keys) = citation_keys(&after[..end]) {
                    let labels = keys
                        .iter()
                        .map(|key| format!("label(\"{}\")", esc_string(key)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&emit_text(&rest[..next.0]));
                    if keys.len() == 1 {
                        out.push_str(&format!("#cite({labels})"));
                    } else {
                        out.push_str(&format!("#md-cite-group(({labels}))"));
                    }
                    rest = &after[end + CITATION_CLOSE_TOKEN.len()..];
                    continue;
                }
            }
        }

        let after = &rest[next.0 + 2..];
        if let Some(end_rel) = after.find("==") {
            let inner = &after[..end_rel];
            if !inner.is_empty() && !inner.starts_with('=') && !inner.ends_with('=') {
                out.push_str(&emit_text(&rest[..next.0]));
                out.push_str("#highlight[");
                out.push_str(&emit_text(inner));
                out.push(']');
                rest = &after[end_rel + 2..];
                continue;
            }
        }
        out.push_str(&emit_text(&rest[..next.0 + 1]));
        rest = &rest[next.0 + 1..];
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
    // Typst raw text has no escape sequences, so a literal backtick cannot be
    // escaped inside `…` — it would close the raw early and leave the rest of
    // the document unbalanced. The function form takes a normal string.
    if literal.contains('`') {
        format!("#raw(\"{}\")", esc_string(literal))
    } else {
        format!("`{}`", literal)
    }
}

/// Math is delegated to the Typst package's `md-math` helper (mitex-backed),
/// so the engine carries no LaTeX->Typst conversion.
fn render_math(display: bool, latex: &str, alignment: Alignment) -> String {
    let math = format!("#md-math({display}, \"{}\")", esc_string(latex.trim()));
    if display {
        format!("#align({})[#box[{math}]]", alignment.typst())
    } else {
        math
    }
}

pub(crate) fn heading_parts(text: &str) -> (String, Option<String>) {
    let trimmed = text.trim();
    let Some(open) = trimmed.rfind(" {#") else {
        return (trimmed.to_string(), None);
    };
    if !trimmed.ends_with('}') {
        return (trimmed.to_string(), None);
    }
    let id = &trimmed[open + 3..trimmed.len() - 1];
    let mut chars = id.chars();
    let valid = chars.next().is_some_and(char::is_alphanumeric)
        && chars.all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
        && !id.to_ascii_lowercase().starts_with("md2pdf-");
    if !valid {
        return (trimmed.to_string(), None);
    }
    (trimmed[..open].trim_end().to_string(), Some(id.to_string()))
}

pub(crate) fn unique_heading_id(
    slugs: &mut HashMap<String, usize>,
    text: &str,
    custom: Option<&str>,
) -> String {
    let mut base = custom.map(str::to_string).unwrap_or_else(|| {
        text.chars()
            .flat_map(|c| {
                if c.is_alphanumeric() {
                    c.to_lowercase().collect::<Vec<_>>()
                } else {
                    vec!['-']
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    });
    while let Some(rest) = base.strip_prefix("md2pdf-") {
        base = rest.to_string();
    }
    if base.is_empty() {
        base = "section".to_string();
    }
    let n = slugs.entry(base.clone()).or_insert(0);
    *n += 1;
    if *n == 1 {
        base
    } else {
        format!("{base}-{n}")
    }
}

/// Reject schemes that execute. Relative paths and fragments pass through.
///
/// An allowlist, not a blocklist: comrak has already decoded entities in the
/// destination, so the usual `&#106;avascript:` and case tricks arrive
/// normalised, and anything unrecognised is refused rather than guessed at.
pub(crate) fn safe_url(url: &str) -> Option<&str> {
    let trimmed = url.trim();
    let scheme: String = trimmed
        .chars()
        .take_while(|c| *c != ':' && *c != '/' && *c != '?' && *c != '#')
        .collect();
    let has_scheme = trimmed.len() > scheme.len() && trimmed[scheme.len()..].starts_with(':');
    if has_scheme {
        let s = scheme.trim().to_ascii_lowercase();
        if !matches!(s.as_str(), "http" | "https" | "mailto" | "tel" | "ftp") {
            return None;
        }
    }
    Some(trimmed)
}

fn render_link(url: &str, label: &str) -> String {
    let escaped_url = esc_text(url);
    let label = if label.trim().is_empty() || label == escaped_url {
        format!("#text(\"{}\")", esc_string(url))
    } else {
        label.to_string()
    };
    // A rejected scheme keeps its text and loses its link, exactly as in the
    // HTML renderer. Typst turns a link into a PDF annotation and typst.ts
    // into an `<a>` in the preview SVG, which the editor adopts into the page
    // itself — so `javascript:` must not reach either.
    match safe_url(url) {
        Some(href) if href.starts_with('#') && href.len() > 1 => {
            format!("#md-jump(\"{}\")[{}]", esc_string(&href[1..]), label)
        }
        Some(href) => format!("#link(\"{}\")[{}]", esc_string(href), label),
        None => label,
    }
}

/// Port of `renderImage`: HackMD `=WxH` dimension syntax + remote-URL aliasing.
fn render_image(url: &str, title: &str, alt: &str, alignment: Alignment) -> String {
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
    let figure_width = dims
        .as_ref()
        .and_then(|(width, _)| width.as_ref())
        .map(|width| format!("{width}pt"))
        .unwrap_or_else(|| "100%".to_string());

    // Alt text becomes a small centered caption, unless it is just a dim spec.
    let caption = alt.trim();
    let caption_is_dims =
        caption.starts_with('=') || caption.chars().next().is_some_and(|c| c.is_ascii_digit());
    if caption.is_empty() || caption_is_dims {
        return format!("#align({})[{image_call}]", alignment.typst());
    }
    format!(
        "#align({})[\n  #block(width: {figure_width}, breakable: false)[\n    #align(center)[{image_call}]\n    #v(0.3em, weak: true)\n    #align(center, text(size: 0.85em, fill: luma(120), [{}]))\n  ]\n]",
        alignment.typst(),
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

fn frontmatter_image_path(value: &str) -> Option<String> {
    let value = value.trim();
    if !value.starts_with("![") || !value.ends_with(')') {
        return None;
    }
    let open = value.find("](")?;
    let inner = value[open + 2..value.len() - 1].trim();
    if inner.is_empty() {
        return None;
    }

    if let Some(space) = inner.rfind(char::is_whitespace) {
        let (path, tail) = inner.split_at(space);
        let size = tail.trim().trim_start_matches('=');
        if let Some((width, height)) = size.split_once(['x', 'X']) {
            let valid = |part: &str| {
                part.is_empty() || part.chars().all(|c| c.is_ascii_digit() || c == '.')
            };
            if (!width.is_empty() || !height.is_empty()) && valid(width) && valid(height) {
                return Some(path.trim().to_string());
            }
        }
    }
    Some(inner.to_string())
}

/// Every image target in the document, deduplicated, with any `=WxH` size hint
/// stripped. Descends into admonition and spoiler bodies and includes the
/// frontmatter fields that the PDF template renders as images.
///
/// Both hosts read this: the CLI prefetches the remote ones, the HTML renderer
/// embeds the local ones. Parsed rather than string-scanned, so a URL that only
/// appears as an example inside a code span is not fetched over the network.
fn image_targets(src: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let fm = html::Frontmatter::parse(src);
    let cover_image = fm.first("cover-image").or_else(|| fm.first("cover_image"));
    {
        let mut add = |path: String| {
            if !path.is_empty() && seen.insert(path.clone()) {
                found.push(path);
            }
        };

        if let Some(value) = cover_image {
            add(frontmatter_image_path(value).unwrap_or_else(|| value.trim().to_string()));
        }

        for (hyphenated, underscored) in [
            ("cover-logo", "cover_logo"),
            ("header-left", "header_left"),
            ("header-center", "header_center"),
            ("header-right", "header_right"),
            ("footer-left", "footer_left"),
            ("footer-center", "footer_center"),
            ("footer-right", "footer_right"),
        ] {
            if let Some(path) = fm
                .first(hyphenated)
                .or_else(|| fm.first(underscored))
                .and_then(frontmatter_image_path)
            {
                add(path);
            }
        }
    }

    walk_document(src, &mut |value| {
        if let NodeValue::Image(link) = value {
            let (path, _) = html::split_dims(&link.url, &link.title);
            if !path.is_empty() && seen.insert(path.clone()) {
                found.push(path);
            }
        }
    });
    found
}

/// Remote image URLs paired with the `remote/<hash>` alias the host prefetches
/// them to — Typst's sandbox cannot fetch them itself.
fn collect_remote_images(src: &str) -> Vec<(String, String)> {
    image_targets(src)
        .into_iter()
        .filter(|u| is_remote(u))
        .map(|u| {
            let alias = format!("remote/{}", hash_url(&u));
            (u, alias)
        })
        .collect()
}

/// Visit every node of the document in source order, descending into the
/// admonition, spoiler and row bodies that were lifted out before parsing.
///
/// Parsing rather than string-scanning is what keeps an example fence inside a
/// wider fence, or an image path inside a code span, from being mistaken for
/// the real thing.
fn walk_document(src: &str, visit: &mut impl FnMut(&NodeValue)) {
    let pre = preprocess(src);
    let arena = Arena::new();
    let root = parse_document(&arena, &pre.markdown, &build_options());
    let mut stack: Vec<&AstNode> = vec![root];
    let mut nested: Vec<String> = Vec::new();
    while let Some(node) = stack.pop() {
        let value = &node.data.borrow().value;
        if let NodeValue::HtmlBlock(hb) = value {
            if let Some(id) = parse_placeholder(&hb.literal, "admonition") {
                if let Some(a) = pre.admonitions.get(id) {
                    nested.push(a.source.clone());
                }
            } else if let Some(id) = parse_placeholder(&hb.literal, "spoiler") {
                if let Some(s) = pre.spoilers.get(id) {
                    nested.push(s.source.clone());
                }
            }
        }
        visit(value);
        let kids: Vec<&AstNode> = node.children().collect();
        stack.extend(kids.into_iter().rev());
    }
    for source in nested {
        walk_document(&source, visit);
    }
}

/// Every ```` ```mermaid ```` fence in the document, deduplicated.
fn has_math(src: &str) -> bool {
    let mut found = false;
    walk_document(src, &mut |value| {
        found |= matches!(value, NodeValue::Math(_));
    });
    found
}

fn collect_mermaid_sources(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    walk_document(src, &mut |value| {
        if let NodeValue::CodeBlock(cb) = value {
            if cb.info.trim().eq_ignore_ascii_case("mermaid") {
                let code = cb.literal.strip_suffix('\n').unwrap_or(&cb.literal).to_string();
                if seen.insert(code.clone()) {
                    out.push(code);
                }
            }
        }
    });
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_self_referencing_footnote_does_not_recurse_forever() {
        let out = convert_str("a[^n]\n\n[^n]: see[^n]", false);
        assert!(out.contains("#footnote[see]"), "{out}");
    }

    fn widths(md: &str) -> Vec<Vec<usize>> {
        preprocess_table_widths(as_lines(md)).1
    }

    fn stripped(md: &str) -> String {
        join_lines(&preprocess_table_widths(as_lines(md)).0).0
    }

    #[test]
    fn extracts_only_a_trailing_bibtex_block() {
        let source = "Text [@example].\n\n@article{example,\n  title = {Example}\n}\n";
        assert_eq!(
            split_inline_bibliography(source),
            (
                "Text [@example].".to_string(),
                "@article{example,\n  title = {Example}\n}".to_string(),
            )
        );
    }

    #[test]
    fn ignores_bibtex_openers_inside_code_fences() {
        let source = "```bibtex\n@article{example,\n}\n```\n\nAfter.\n";
        assert_eq!(
            split_inline_bibliography(source),
            (source.to_string(), String::new())
        );
    }

    #[test]
    fn leaves_a_malformed_bibtex_opener_in_the_document() {
        let source = "Text.\n\n@article example\n";
        assert_eq!(
            split_inline_bibliography(source),
            (source.to_string(), String::new())
        );
    }

    #[test]
    fn citation_rendering_is_opt_in_and_repeatable() {
        let source = "See [@first], [@missing], and [@first].";
        let ordinary = convert_str(source, false);
        assert!(ordinary.contains(r"\[\@first\]"), "{ordinary}");

        let cited = convert_str_with_citations(source, false, true);
        assert_eq!(cited.matches("#cite(label(\"first\"))").count(), 2);
        assert!(cited.contains("#cite(label(\"missing\"))"), "{cited}");
    }

    #[test]
    fn grouped_citations_become_one_typst_citation() {
        let out = convert_str_with_citations("See [@first, @second].", false, true);
        assert!(
            out.contains("#md-cite-group((label(\"first\"), label(\"second\")))"),
            "{out}"
        );
        assert_eq!(out.matches("#md-cite-group(").count(), 1, "{out}");
    }

    #[test]
    fn malformed_citation_syntax_remains_text() {
        let out = convert_str_with_citations("See [@two keys] and [@].", false, true);
        assert!(out.contains(r"\[\@two keys\]"), "{out}");
        assert!(out.contains(r"\[\@\]"), "{out}");
    }

    #[test]
    fn citations_in_code_remain_literal() {
        let out = convert_str_with_citations("`[@inline]`\n\n```txt\n[@fenced]\n```", false, true);
        assert!(out.contains("`[@inline]`"), "{out}");
        assert!(out.contains("[@fenced]"), "{out}");
        assert!(!out.contains("#cite"), "{out}");
    }

    /// Both renderers refuse the same schemes. They used to disagree: only the
    /// HTML side checked, so `javascript:` reached Typst's link annotation —
    /// and, through typst.ts, an `<a>` the editor adopts into the page itself.
    #[test]
    fn both_renderers_refuse_the_same_link_schemes() {
        for url in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            " javascript:alert(1)",
            "vbscript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "file:///etc/passwd",
        ] {
            assert_eq!(safe_url(url), None, "{url} was allowed");
            let out = convert_str(&format!("[label]({url})"), false);
            assert!(!out.contains("#link("), "{url} became a link:\n{out}");
            assert!(out.contains("label"), "{url} lost its text:\n{out}");
        }
        for url in ["https://e.com/x", "mailto:a@e.com", "./rel.md", "#frag"] {
            assert!(safe_url(url).is_some(), "{url} was refused");
            let out = convert_str(&format!("[label]({url})"), false);
            assert!(
                out.contains("#link(") || out.contains("#md-jump("),
                "{url} lost its link:\n{out}"
            );
        }
    }

    #[test]
    fn autolink_labels_are_literal_typst_text() {
        for (markdown, url) in [
            ("https://a.com/post_1.html", "https://a.com/post_1.html"),
            ("<https://a.com/post_2.html>", "https://a.com/post_2.html"),
        ] {
            let out = convert_str(markdown, false);
            assert_eq!(
                out.trim(),
                format!("#link(\"{url}\")[#text(\"{url}\")]"),
                "URL label can be reparsed as a nested Typst autolink"
            );
        }
    }

    #[test]
    fn headings_and_fragment_links_share_ids() {
        let out = convert_str(
            "[forward](#overview)\n\n## Visible {#overview}\n\n### Visible {#overview}",
            false,
        );
        assert!(out.contains("#md-jump(\"overview\")[forward]"), "{out}");
        assert!(out.contains("== Visible <overview>"), "{out}");
        assert!(out.contains("=== Visible <overview-2>"), "{out}");
        assert!(!out.contains("{#overview}"), "{out}");
    }

    #[test]
    fn invalid_heading_ids_stay_visible() {
        for heading in ["Bad {#two words}", "Bad {#-start}", "Bad {#md2pdf-root}"] {
            let out = convert_str(&format!("## {heading}"), false);
            assert!(out.contains("Bad {\\#"), "{out}");
        }
    }

    #[test]
    fn the_pdf_style_breaks_tables_only_between_rows() {
        let style = include_str!("../../package/styles/modern-tech.typ");
        assert!(style.contains("set table.cell(breakable: false)"));
    }

    /// A URL reaches Typst inside a string literal, and Typst can `read()`.
    /// A breakout there would be worse than an XSS.
    #[test]
    fn a_link_url_cannot_break_out_of_the_typst_string() {
        let out = convert_str(r#"[x](https://e.com/a"+read("/etc/passwd")+")"#, false);
        assert_eq!(
            out.trim(),
            r#"#link("https://e.com/a\"+read(\"/etc/passwd\")+\"")[x]"#
        );
        // Every quote inside the argument is escaped, so the literal ends
        // exactly where the renderer put its closing delimiter.
        let arg = out.trim().trim_start_matches("#link(\"");
        let unescaped = arg
            .char_indices()
            .filter(|&(i, c)| c == '"' && !arg[..i].ends_with('\\'))
            .count();
        assert_eq!(unescaped, 1, "extra unescaped quote in {arg}");
    }

    #[test]
    fn recognizes_the_additional_callout_kinds() {
        for kind in ["caution", "important"] {
            let out = convert_str(&format!(":::{kind}\nBody\n:::\n"), false);
            assert!(
                out.contains(&format!("#admonition(kind: \"{kind}\")")),
                "{out}"
            );
        }
    }

    #[test]
    fn counts_pluses_per_column() {
        assert_eq!(widths("| a | b |\n| --- | ---++ |\n"), vec![vec![1, 3]]);
    }

    #[test]
    fn accepts_single_dash_and_alignment_colons() {
        assert_eq!(widths("| a | b | c |\n| - | :-+ | -:+ |\n"), vec![vec![1, 2, 2]]);
    }

    #[test]
    fn accepts_missing_outer_pipes() {
        assert_eq!(widths("a | b\n--- | ---+\n"), vec![vec![1, 2]]);
    }

    #[test]
    fn keeps_the_blockquote_prefix() {
        let out = stripped("> | a | b |\n> | - | -+ |\n");
        assert_eq!(widths("> | a | b |\n> | - | -+ |\n"), vec![vec![1, 2]]);
        assert!(out.lines().all(|l| l.starts_with('>')), "{out}");
    }

    #[test]
    fn keeps_the_list_item_indent() {
        let out = stripped("1. step\n\n   | a | b |\n   | - | -+ |\n");
        assert!(out.contains("   <!--tablewidths:0-->"), "{out}");
    }

    #[test]
    fn ignores_code_blocks() {
        let fenced = "```\n| a | b |\n| - | -+ |\n```\n";
        assert!(widths(fenced).is_empty());
        assert_eq!(stripped(fenced), fenced);
        let indented = "text\n\n    | a | b |\n    | - | -+ |\n";
        assert!(widths(indented).is_empty());
        assert_eq!(stripped(indented), indented);
    }

    #[test]
    fn ignores_rows_that_are_not_separators() {
        assert!(widths("| a | b |\n| c+ | d |\n").is_empty());
        assert!(widths("| --- | ---+ |\n").is_empty());
    }

    #[test]
    fn inline_code_with_a_backtick_uses_the_function_form() {
        assert_eq!(render_inline_code("a`b"), "#raw(\"a`b\")");
        assert_eq!(render_inline_code("ab"), "`ab`");
    }

    #[test]
    fn keeps_visuals_centered_without_a_directive() {
        assert_eq!(
            render_image("image.png", "=80x", "", Alignment::Center),
            "#align(center)[#image(\"image.png\", width: 80pt)]"
        );
        assert_eq!(
            render_math(true, "x = 1", Alignment::Center),
            "#align(center)[#box[#md-math(true, \"x = 1\")]]"
        );
    }

    #[test]
    fn aligns_a_captioned_image_as_one_sized_figure() {
        let out = convert_str(":::left\n![Caption](image.png \"=80x\")\n:::\n", false);
        assert!(out.contains("#align(left)[\n  #align(left)["), "{out}");
        assert!(
            out.contains("#block(width: 80pt, breakable: false)"),
            "{out}"
        );
        assert!(out.contains("#align(center, text(size: 0.85em"), "{out}");
    }

    #[test]
    fn nearest_alignment_reaches_visuals_in_nested_containers() {
        let out = convert_str(
            "::::::right\n:::::tip\n![](right.png \"=80x\")\n\n:::center\n![](center.png \"=80x\")\n:::\n:::::\n::::::\n",
            false,
        );
        assert!(out.contains("#align(right)["), "{out}");
        assert!(out.contains("#admonition(kind: \"tip\")"), "{out}");
        assert!(
            out.contains("#align(right)[#image(\"right.png\", width: 80pt)]"),
            "{out}"
        );
        assert!(
            out.contains("#align(center)[#image(\"center.png\", width: 80pt)]"),
            "{out}"
        );
    }

    #[test]
    fn inherited_alignment_reaches_rows_and_spoilers() {
        let row = convert_str(
            ":::::right\n::::row\n![](row.png \"=80x\")\n::::\n:::::\n",
            false,
        );
        assert!(row.contains("#grid("), "{row}");
        assert!(
            row.contains("#align(right)[#image(\"row.png\", width: 80pt)]"),
            "{row}"
        );

        let spoiler = convert_str(
            ":::left\n+++++ Summary\n![](spoiler.png \"=80x\")\n+++++\n:::\n",
            false,
        );
        assert!(spoiler.contains("#spoiler(summary: \"Summary\")"), "{spoiler}");
        assert!(
            spoiler.contains("#align(left)[#image(\"spoiler.png\", width: 80pt)]"),
            "{spoiler}"
        );
    }

    #[test]
    fn aligns_display_math_and_mermaid_but_not_structural_blocks() {
        let out = convert_str(
            ":::right\n$$x = 1$$\n\n```mermaid\ngraph LR\n```\n\n```txt\ncode\n```\n\n| a | b |\n| - | - |\n| c | d |\n:::\n",
            false,
        );
        assert!(
            out.contains("#align(right)[#box[#md-math(true, \"x = 1\")]]"),
            "{out}"
        );
        assert!(
            out.contains("#align(right)[#md-mermaid(\"graph LR\")]"),
            "{out}"
        );
        assert!(out.contains("```txt\n  code\n  ```"), "{out}");
        assert!(out.contains("#table("), "{out}");
    }
}
