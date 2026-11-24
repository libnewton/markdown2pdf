//! Pre-parse passes: the custom block syntax comrak cannot parse is lifted
//! out of the source before either renderer sees it.

use crate::build_options;
use comrak::nodes::NodeValue;
use comrak::{parse_document, Arena};
use std::collections::HashSet;

/// Token marking a preserved run of 3+ blank lines.
const EXTRA_BLANK_LINE_TOKEN: &str = "[[md2pdf-blank-line]]";
pub(crate) const CITATION_OPEN_TOKEN: &str = "\u{e000}md2pdf-cite:";
pub(crate) const CITATION_CLOSE_TOKEN: &str = "\u{e001}";

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
    let text = lines
        .iter()
        .map(|(l, _)| l.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (text, lines.iter().map(|(_, o)| *o).collect())
}

/// Map origins expressed in `base`'s coordinates into `base`'s own, for the
/// passes that re-run over a block's extracted body.
pub(crate) fn rebase(local: &[u32], base: &[u32]) -> Vec<u32> {
    local
        .iter()
        .map(|&l| {
            if l == 0 {
                0
            } else {
                base.get(l as usize - 1).copied().unwrap_or(0)
            }
        })
        .collect()
}

pub(crate) struct Admonition {
    pub(crate) kind: String,
    pub(crate) title: String,
    pub(crate) source: String,
    pub(crate) origin: Vec<u32>,
}

pub(crate) struct Spoiler {
    pub(crate) summary: String,
    pub(crate) source: String,
    pub(crate) origin: Vec<u32>,
}

pub(crate) struct Preprocessed {
    pub(crate) markdown: String,
    /// One entry per line of `markdown`, naming its line in the original source.
    pub(crate) origin: Vec<u32>,
    pub(crate) admonitions: Vec<Admonition>,
    pub(crate) spoilers: Vec<Spoiler>,
    /// Per-table column-width multipliers, indexed by `<!--tablewidths:N-->`.
    pub(crate) table_widths: Vec<Vec<usize>>,
    /// Lettered and roman list numbering, indexed by `<!--liststyle:N-->`.
    pub(crate) list_styles: Vec<ListStyle>,
}

/// Split a trailing inline BibTeX section from Markdown. Entry openers inside
/// fenced code are ignored, so examples remain ordinary document content.
pub(crate) fn split_inline_bibliography(src: &str) -> (String, String) {
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

pub(crate) fn preprocess_citations(src: &str) -> String {
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

pub(crate) fn citation_keys(group: &str) -> Option<Vec<&str>> {
    let mut keys = Vec::new();
    for part in group.split([',', ';']) {
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

/// The `:::` words that lay their body out rather than box it.
const LAYOUT_KINDS: &[&str] = &["left", "center", "right", "row"];

/// A `:::kind` this engine renders: a callout from the shared palette or a
/// layout directive.
fn is_block_kind(kind: &str) -> bool {
    LAYOUT_KINDS.contains(&kind)
        || crate::html::tokens::ADMONITIONS
            .iter()
            .any(|a| a.kind == kind)
}

/// Extract `:::kind` and `+++++` blocks, replacing each with an HTML-comment
/// placeholder that comrak parses as a standalone `HtmlBlock`.
pub(crate) fn preprocess(src: &str) -> Preprocessed {
    let normalized = crate::frontmatter::blank_frontmatter(src).replace("\r\n", "\n");
    let l0 = preprocess_extensions(preprocess_blank_lines(as_lines(&normalized)));
    let (l0, list_styles) = preprocess_lettered_lists(l0);
    let mut spoilers = Vec::new();
    let (l1, admonitions) = preprocess_admonitions(l0, &mut spoilers);
    let l2 = preprocess_spoilers(l1, &mut spoilers);
    let (l3, table_widths) = preprocess_table_widths(l2);
    let (markdown, origin) = join_lines(&l3);
    Preprocessed {
        markdown,
        origin,
        admonitions,
        spoilers,
        table_widths,
        list_styles,
    }
}

/// Collapse a run of 3+ blank lines into a `[[md2pdf-blank-line]]` token so the
/// extra vertical space survives parsing. Skips fenced code; only fires between
/// two "preservable" lines (not list/quote/table/rule/fence/pagebreak).
fn preprocess_blank_lines(lines: Vec<Line>) -> Vec<Line> {
    if !lines
        .windows(3)
        .any(|w| w.iter().all(|(l, _)| l.trim().is_empty()))
    {
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
    !(digits > 0 && t[digits..].starts_with('.'))
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
            out.push((format!("{prefix}{}", placeholder("tablewidths", id)), 0));
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
///
/// An opener may sit inside a list item or a blockquote; the block is then
/// lifted out of that container and its placeholder put back in it.
/// `:::spoiler Title` is HedgeDoc's spelling of a spoiler and becomes one. An
/// opener without a closer is not a block at all and stays literal text.
fn preprocess_admonitions(
    lines: Vec<Line>,
    spoilers: &mut Vec<Spoiler>,
) -> (Vec<Line>, Vec<Admonition>) {
    if !lines.iter().any(|(l, _)| l.contains(":::")) {
        return (lines, Vec::new());
    }
    let code = code_block_lines(&join_lines(&lines).0);
    let is_code = |i: usize| code.contains(&(i + 1));
    let mut blocks: Vec<Admonition> = Vec::new();
    let mut out: Vec<Line> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (prefix, rest) = split_prefix(&lines[i].0);
        let open = (!is_code(i)).then(|| parse_admonition_open(rest)).flatten();
        let Some((fence_len, kind, title)) = open else {
            out.push(lines[i].clone());
            i += 1;
            continue;
        };
        let close = ((i + 1)..lines.len()).find(|&j| {
            !is_code(j) && is_colon_closer(strip_container(&lines[j].0, prefix), fence_len)
        });
        let Some(close) = close else {
            out.push(lines[i].clone());
            i += 1;
            continue;
        };
        let body: Vec<Line> = lines[(i + 1)..close]
            .iter()
            .map(|(l, o)| (strip_container(l, prefix).to_string(), *o))
            .collect();
        // The body is re-parsed as its own document, so it keeps its own
        // origins to be rebased against these when it is rendered.
        let (source, origin) = join_lines(&body);
        let placeholder = if kind == "spoiler" {
            spoilers.push(Spoiler {
                summary: if title.is_empty() {
                    "spoiler".to_string()
                } else {
                    title
                },
                source,
                origin,
            });
            placeholder("spoiler", spoilers.len() - 1)
        } else {
            blocks.push(Admonition {
                kind,
                title,
                source,
                origin,
            });
            placeholder("admonition", blocks.len() - 1)
        };
        push_placeholder(&mut out, prefix, placeholder);
        i = close + 1;
    }
    (out, blocks)
}

/// Spoilers: a fence of three or more `+`, closed by a fence of the same
/// length, so Outline's `++++` toggle can hold a `+++` one. The trailing text
/// on the opener, or else the first non-blank inner line, is the summary; the
/// rest is the body. A fence of five or more closes on any such fence, as it
/// always has.
fn preprocess_spoilers(lines: Vec<Line>, blocks: &mut Vec<Spoiler>) -> Vec<Line> {
    if !lines.iter().any(|(l, _)| l.contains("+++")) {
        return lines;
    }
    let code = code_block_lines(&join_lines(&lines).0);
    let is_code = |i: usize| code.contains(&(i + 1));
    let mut out: Vec<Line> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let open = (!is_code(i))
            .then(|| parse_spoiler_open(&lines[i].0))
            .flatten();
        let close = open.as_ref().and_then(|(plus, _)| {
            ((i + 1)..lines.len()).find(|&j| !is_code(j) && is_spoiler_closer(&lines[j].0, *plus))
        });
        let (Some((_, inline)), Some(close)) = (open, close) else {
            out.push(lines[i].clone());
            i += 1;
            continue;
        };
        let mut body: Vec<Line> = lines[(i + 1)..close].to_vec();
        let summary = if !inline.is_empty() {
            inline
        } else {
            match body.iter().position(|(l, _)| !l.trim().is_empty()) {
                Some(k) => {
                    let s = body[k].0.trim().to_string();
                    body.drain(..=k);
                    s
                }
                None => String::new(),
            }
        };
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
        push_placeholder(&mut out, "", placeholder("spoiler", blocks.len() - 1));
        i = close + 1;
    }
    out
}

/// Opens and closes a HedgeDoc byline (`[name=…] [time=…]`), already reduced
/// to its display text. Private-use characters, like the citation tokens.
pub(crate) const BYLINE_OPEN: char = '\u{E004}';
pub(crate) const BYLINE_CLOSE: char = '\u{E005}';

/// Line-level syntax from HedgeDoc and Outline, rewritten into what the parser
/// and the renderers already understand. Code fences are left alone.
fn preprocess_extensions(lines: Vec<Line>) -> Vec<Line> {
    let candidate = |l: &str| {
        l.trim() == "\\"
            || l.contains("{%")
            || l.contains("![")
            || BYLINE_TAGS.iter().any(|t| l.contains(t))
    };
    if !lines.iter().any(|(l, _)| candidate(l)) {
        return lines;
    }
    // Asking the parser is what tells an indented code block from a list
    // item's continuation; fences come along with it.
    let code = code_block_lines(&join_lines(&lines).0);
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for (i, (line, origin)) in lines.iter().enumerate() {
        if code.contains(&(i + 1)) || !candidate(line) {
            out.push((line.clone(), *origin));
            continue;
        }
        let blank = |j: Option<usize>| {
            j.and_then(|j| lines.get(j))
                .is_none_or(|(l, _)| l.trim().is_empty())
        };
        let alone = blank(i.checked_sub(1)) && blank(Some(i + 1));
        // Outline writes an empty paragraph as a line holding only `\`.
        let after_blank =
            blank(i.checked_sub(1)) || out.last().is_some_and(|(l, _)| l == EXTRA_BLANK_LINE_TOKEN);
        if line.trim() == "\\" && after_blank {
            out.push((String::new(), 0));
            out.push((EXTRA_BLANK_LINE_TOKEN.to_string(), *origin));
            out.push((String::new(), 0));
            continue;
        }
        let line = match embed_link(line).filter(|_| alone) {
            Some(link) => link,
            None => outside_code_spans(line, |text| bylines(&sized_image_targets(text))),
        };
        out.push((line, *origin));
    }
    out
}

/// Apply `rewrite` to the parts of a line outside inline code spans, which
/// stay verbatim. A backtick run opens a span that the next run of the same
/// length closes; an unclosed run is plain text.
fn outside_code_spans(line: &str, rewrite: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(line.len());
    let mut text_start = 0;
    let mut i = 0;
    let bytes = line.as_bytes();
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
        let fence = &line[i..i + run];
        let close = line[i + run..]
            .match_indices(fence)
            .map(|(at, _)| i + run + at)
            .find(|&at| {
                line.as_bytes().get(at + run) != Some(&b'`') && line.as_bytes()[at - 1] != b'`'
            });
        match close {
            Some(end) => {
                out.push_str(&rewrite(&line[text_start..i]));
                out.push_str(&line[i..end + run]);
                i = end + run;
                text_start = i;
            }
            None => i += run,
        }
    }
    out.push_str(&rewrite(&line[text_start..]));
    out
}

/// A HedgeDoc embed paragraph, `{%youtube ID %}`, as a labelled link.
fn embed_link(line: &str) -> Option<String> {
    let inner = line.trim().strip_prefix("{%")?.strip_suffix("%}")?.trim();
    let (kind, arg) = inner.split_once(char::is_whitespace)?;
    let arg = arg.trim();
    if arg.is_empty() || arg.contains(char::is_whitespace) || arg.contains(['<', '>']) {
        return None;
    }
    let (label, url) = match kind.to_ascii_lowercase().as_str() {
        "youtube" => ("YouTube", format!("https://www.youtube.com/watch?v={arg}")),
        "vimeo" => ("Vimeo", format!("https://vimeo.com/{arg}")),
        "gist" => ("Gist", format!("https://gist.github.com/{arg}")),
        "slideshare" => ("SlideShare", format!("https://www.slideshare.net/{arg}")),
        "speakerdeck" => ("Speaker Deck", format!("https://speakerdeck.com/{arg}")),
        "pdf" => ("PDF", arg.to_string()),
        _ => return None,
    };
    let text: String = arg
        .chars()
        .flat_map(|c| {
            let escape = c.is_ascii_punctuation();
            escape.then_some('\\').into_iter().chain([c])
        })
        .collect();
    Some(format!("[{label}: {text}](<{url}>)"))
}

/// HedgeDoc's `![a](pic.png =200x100)`: a size inside the parentheses without
/// the angle brackets CommonMark needs to allow the space.
fn sized_image_targets(line: &str) -> String {
    if !line.contains("![") {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + 4);
    let mut rest = line;
    while let Some(img) = rest.find("![") {
        let Some(open) = rest[img..].find("](").map(|o| img + o + 2) else {
            break;
        };
        out.push_str(&rest[..open]);
        rest = &rest[open..];
        let Some(close) = rest.find(')') else { break };
        let target = &rest[..close];
        let mut parts = target.split_whitespace();
        if let (Some(url), Some(size), None) = (parts.next(), parts.next(), parts.next()) {
            if !url.starts_with('<') && size.starts_with('=') && crate::parse_dims(size).is_some() {
                out.push_str(&format!("<{url} {size}>"));
                rest = &rest[close..];
                continue;
            }
        }
        out.push_str(target);
        rest = &rest[close..];
    }
    out.push_str(rest);
    out
}

const BYLINE_TAGS: [&str; 3] = ["[name=", "[time=", "[color="];

/// A byline tag at the start of `s`: (kind, value, length).
fn byline_tag(s: &str) -> Option<(&'static str, &str, usize)> {
    let prefix = BYLINE_TAGS.into_iter().find(|t| s.starts_with(t))?;
    let end = s.find(']')?;
    Some((
        &prefix[1..prefix.len() - 1],
        s[prefix.len()..end].trim(),
        end + 1,
    ))
}

/// HedgeDoc's byline tags. A run of `[name=…]`, `[time=…]` and `[color=…]`
/// separated only by spaces becomes one byline, "— name, time"; the colour
/// is dropped.
fn bylines(line: &str) -> String {
    if !BYLINE_TAGS.iter().any(|t| line.contains(t)) {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find('[') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let mut parts = Vec::new();
        let mut consumed = 0;
        let mut cursor = rest;
        while let Some((kind, value, len)) = byline_tag(cursor) {
            if kind != "color" && !value.is_empty() {
                parts.push(value);
            }
            let skipped = cursor[len..].len() - cursor[len..].trim_start().len();
            consumed += len;
            cursor = &cursor[len..];
            if byline_tag(cursor.trim_start()).is_some() {
                consumed += skipped;
                cursor = cursor.trim_start();
            } else {
                break;
            }
        }
        if consumed == 0 {
            out.push('[');
            rest = &rest[1..];
            continue;
        }
        rest = &rest[consumed..];
        if parts.is_empty() {
            // A lone colour leaves nothing behind, not even its gap.
            rest = rest.trim_start_matches(' ');
        } else {
            out.push(BYLINE_OPEN);
            out.push_str(&format!("— {}", parts.join(", ")));
            out.push(BYLINE_CLOSE);
        }
    }
    out.push_str(rest);
    out
}

/// Lettered and roman ordered lists (`a.`, `A.`, `i.`, `I.`), which CommonMark
/// does not know. A run of two or more consecutive, correctly sequenced items
/// after a blank line becomes a numbered list, preceded by a placeholder that
/// tells the renderers how to number it.
fn preprocess_lettered_lists(lines: Vec<Line>) -> (Vec<Line>, Vec<ListStyle>) {
    let mut styles = Vec::new();
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    let mut fence: Option<(char, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        if let Some((fc, fl)) = fence_marker(&lines[i].0) {
            match fence {
                None => fence = Some((fc, fl)),
                Some((c, l)) if c == fc && fl >= l => fence = None,
                _ => {}
            }
        }
        let after_blank = i == 0 || lines[i - 1].0.trim().is_empty();
        let run = if fence.is_none() && after_blank {
            lettered_run(&lines[i..])
        } else {
            None
        };
        let Some((style, items)) = run else {
            out.push(lines[i].clone());
            i += 1;
            continue;
        };
        let (prefix, _) = split_prefix(&lines[i].0);
        styles.push(style);
        out.push((
            format!("{prefix}{}", placeholder("liststyle", styles.len() - 1)),
            0,
        ));
        for (k, (line, origin)) in lines[i..i + items].iter().enumerate() {
            let (prefix, body) = split_prefix(line);
            let text = body.split_once(". ").map_or("", |(_, t)| t);
            out.push((format!("{prefix}{}. {text}", k + style.start), *origin));
        }
        i += items;
    }
    (out, styles)
}

/// How a lettered or roman list counts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ListStyle {
    /// `a`, `A`, `i` or `I`.
    pub(crate) kind: char,
    pub(crate) start: usize,
}

/// The longest run of items at the start of `lines` that count up from the
/// first one: (style, number of lines).
fn lettered_run(lines: &[Line]) -> Option<(ListStyle, usize)> {
    let marker = |line: &str| -> Option<String> {
        let (_, body) = split_prefix(line);
        let (m, rest) = body.split_once(". ")?;
        (!rest.trim().is_empty() && !m.is_empty() && m.chars().all(|c| c.is_ascii_alphabetic()))
            .then(|| m.to_string())
    };
    let first = marker(&lines[0].0)?;
    let kind = match first.as_str() {
        "i" => 'i',
        "I" => 'I',
        m if m.len() == 1 && m.chars().all(|c| c.is_ascii_lowercase()) => 'a',
        m if m.len() == 1 => 'A',
        m if roman_value(m).is_some() => {
            if m.chars().all(|c| c.is_ascii_lowercase()) {
                'i'
            } else {
                'I'
            }
        }
        _ => return None,
    };
    let value = |m: &str| -> Option<usize> {
        match kind {
            'a' => (m.len() == 1 && m.chars().all(|c| c.is_ascii_lowercase()))
                .then(|| (m.as_bytes()[0] - b'a') as usize + 1),
            'A' => (m.len() == 1 && m.chars().all(|c| c.is_ascii_uppercase()))
                .then(|| (m.as_bytes()[0] - b'A') as usize + 1),
            'i' => m
                .chars()
                .all(|c| c.is_ascii_lowercase())
                .then(|| roman_value(m))
                .flatten(),
            _ => m
                .chars()
                .all(|c| c.is_ascii_uppercase())
                .then(|| roman_value(m))
                .flatten(),
        }
    };
    let start = value(&first)?;
    let mut count = 1;
    while let Some(next) = lines.get(count).and_then(|(l, _)| marker(l)) {
        if value(&next) != Some(start + count) {
            break;
        }
        count += 1;
    }
    (count >= 2).then_some((ListStyle { kind, start }, count))
}

fn roman_value(s: &str) -> Option<usize> {
    let digit = |c: char| match c.to_ascii_lowercase() {
        'i' => Some(1),
        'v' => Some(5),
        'x' => Some(10),
        'l' => Some(50),
        'c' => Some(100),
        _ => None,
    };
    let values: Vec<usize> = s.chars().map(digit).collect::<Option<_>>()?;
    // Right to left: a digit smaller than the largest one after it subtracts.
    let (mut total, mut largest) = (0, 0);
    for v in values.into_iter().rev() {
        if v < largest {
            total -= v;
        } else {
            total += v;
            largest = v;
        }
    }
    (total > 0).then_some(total)
}

/// Marks the engine's own placeholders. A private-use character the author
/// will not type, so a literal `<!--admonition:0-->` in the source is just a
/// comment, never a second copy of a block.
const PLACEHOLDER_MARK: char = '\u{E002}';

fn placeholder(kind: &str, id: usize) -> String {
    format!("<!--{PLACEHOLDER_MARK}{kind}:{id}-->")
}

/// Put a block's placeholder where the block was. At the top level it gets
/// blank lines of its own; inside a container it keeps the container prefix
/// and no blank lines, which would loosen an enclosing list.
fn push_placeholder(out: &mut Vec<Line>, prefix: &str, placeholder: String) {
    if prefix.is_empty() {
        out.push((String::new(), 0));
        out.push((placeholder, 0));
        out.push((String::new(), 0));
    } else {
        out.push((format!("{prefix}{placeholder}"), 0));
    }
}

/// A body line with the container prefix of its block's opener removed. A
/// line that lacks the full prefix (a blank line in a quote, a lazy
/// continuation) loses what it has of it.
fn strip_container<'l>(line: &'l str, prefix: &str) -> &'l str {
    if let Some(rest) = line.strip_prefix(prefix) {
        return rest;
    }
    if let Some(rest) = line
        .strip_prefix(prefix.trim_end())
        .filter(|_| !prefix.trim().is_empty())
    {
        return rest;
    }
    let max = prefix.len();
    let cut = line
        .char_indices()
        .take_while(|(i, c)| *i < max && (*c == ' ' || *c == '\t' || *c == '>'))
        .count();
    &line[cut..]
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
    if kind != "spoiler" && !is_block_kind(&kind) {
        return None;
    }
    Some((fence_len, kind, rest[end..].trim().to_string()))
}

/// A line of `fence_len`+ colons and nothing else closes an admonition.
fn is_colon_closer(line: &str, fence_len: usize) -> bool {
    let t = line.trim_end();
    t.len() >= fence_len && !t.is_empty() && t.bytes().all(|b| b == b':')
}

/// Parse a `+++ summary` opener (up to 3 leading spaces) -> (fence length,
/// the possibly empty trailing summary).
fn parse_spoiler_open(line: &str) -> Option<(usize, String)> {
    let bytes = line.as_bytes();
    let spaces = bytes.iter().take_while(|&&b| b == b' ').count();
    if spaces > 3 {
        return None;
    }
    let plus = bytes[spaces..].iter().take_while(|&&b| b == b'+').count();
    if plus < 3 {
        return None;
    }
    Some((plus, line[spaces + plus..].trim().to_string()))
}

/// A line of pluses alone closes the spoiler its length matches.
fn is_spoiler_closer(line: &str, fence_len: usize) -> bool {
    let bytes = line.as_bytes();
    let spaces = bytes.iter().take_while(|&&b| b == b' ').count();
    if spaces > 3 {
        return false;
    }
    let plus = bytes[spaces..].iter().take_while(|&&b| b == b'+').count();
    let matched = plus == fence_len || (fence_len >= 5 && plus >= 5);
    matched && line[spaces + plus..].trim().is_empty()
}

/// Recover the block id from an engine placeholder comrak parsed as an
/// `HtmlBlock`.
pub(crate) fn parse_placeholder(literal: &str, kind: &str) -> Option<usize> {
    let inner = literal
        .trim()
        .strip_prefix("<!--")?
        .strip_prefix(PLACEHOLDER_MARK)?
        .strip_suffix("-->")?;
    inner
        .strip_prefix(kind)?
        .strip_prefix(':')?
        .trim()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn counts_pluses_per_column() {
        assert_eq!(widths("| a | b |\n| --- | ---++ |\n"), vec![vec![1, 3]]);
    }

    #[test]
    fn accepts_single_dash_and_alignment_colons() {
        assert_eq!(
            widths("| a | b | c |\n| - | :-+ | -:+ |\n"),
            vec![vec![1, 2, 2]]
        );
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
        assert!(out.contains("   <!--\u{e002}tablewidths:0-->"), "{out}");
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
}
