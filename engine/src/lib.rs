//! md2pdf engine — Markdown -> Typst markup, as a Typst WASM plugin.
//!
//! Parsing is done by `comrak` (CommonMark + GFM). Custom HackMD-flavoured
//! syntax that comrak does not know (`:::` admonitions, `+++++` spoilers,
//! `==mark==`) is handled by the pre-parse passes in `preprocess.rs` and by
//! post-parse text scanning. `typst.rs` and `html/` are the two renderers;
//! this module holds the plugin ABI, the asset discovery and what both share.

mod emoji;
mod frontmatter;
mod html;
mod preprocess;
mod rawhtml;
mod typst;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, Options};
use emoji::match_emoji;
use preprocess::{parse_placeholder, preprocess, split_inline_bibliography};
use std::collections::{HashMap, HashSet};
use wasm_minimal_protocol::*;

initiate_protocol!();

/// Convert Markdown (UTF-8 bytes) to Typst markup (UTF-8 bytes).
/// `strip_h1` non-empty drops a leading level-1 heading; `citations` non-empty
/// enables `[@key]` citation rendering for an extracted inline bibliography;
/// `remote_dir` names the directory of the remote-image aliases.
#[wasm_func]
pub fn convert(
    markdown: &[u8],
    strip_h1: &[u8],
    citations: &[u8],
    remote_dir: &[u8],
) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    let mut doc = typst::Doc::new(!citations.is_empty(), remote_dir_of(remote_dir)?);
    Ok(typst::convert(src, !strip_h1.is_empty(), &mut doc).into_bytes())
}

/// The remote-image alias directory a host asked for, `remote` by default.
fn remote_dir_of(raw: &[u8]) -> Result<&str, String> {
    let dir = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    Ok(if dir.is_empty() {
        DEFAULT_REMOTE_DIR
    } else {
        dir
    })
}

const DEFAULT_REMOTE_DIR: &str = "remote";

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
    let root = parse(&arena, &pre.markdown);
    let children: Vec<&AstNode> = root.children().collect();
    let text = match leading_h1_index(&children) {
        Some(i) => heading_parts(&plain_text(children[i])).0,
        None => String::new(),
    };
    Ok(text.into_bytes())
}

/// List the Twemoji codepoints the document references (unicode emoji +
/// `:shortcodes:`), one per line. The host/worker fetches the matching SVGs.
#[wasm_func]
pub fn twemojis(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    Ok(collect_twemoji_codepoints(src).join("\n").into_bytes())
}

/// The frontmatter block as a JSON object with normalised keys (lowercase,
/// `_` read as `-`); `{}` when the document has none.
#[wasm_func]
pub fn frontmatter(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    Ok(frontmatter::Frontmatter::parse(src).to_json().into_bytes())
}

/// The shared design tokens as TOML: callout colours and labels, plus the base
/// palette. The stylesheet bakes the same table in, so the Typst templates and
/// the HTML output cannot drift apart.
#[wasm_func]
pub fn tokens() -> Result<Vec<u8>, String> {
    Ok(html::tokens_toml().into_bytes())
}

/// Everything an HTML render needs the host to fetch, from one parse.
///
/// One `kind<TAB>value…` line per asset, where `kind` is `image`, `remote`
/// (`url<TAB>alias`), `emoji`, `font` or `mermaid` (`key<TAB>escaped-source`).
#[wasm_func]
pub fn html_assets(markdown: &[u8]) -> Result<Vec<u8>, String> {
    let src = std::str::from_utf8(markdown).map_err(|e| e.to_string())?;
    let found = discover(src);
    let mut out = String::new();
    for path in found.local_images() {
        out.push_str("image\t");
        out.push_str(path);
        out.push('\n');
    }
    for (url, alias) in found.remotes(DEFAULT_REMOTE_DIR) {
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
    if found.math {
        out.push_str("font\tfonts/math.woff2\nfont\tfonts/math-alpha.woff2\n");
    }
    for code in &found.mermaid {
        out.push_str("mermaid\t");
        out.push_str(&html::mermaid_key(code));
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

/// Index of a leading level-1 heading among root children.
fn leading_h1_index(children: &[&AstNode]) -> Option<usize> {
    let first = children.first()?;
    matches!(&first.data.borrow().value, NodeValue::Heading(h) if h.level == 1).then_some(0)
}

/// A formatting tag opening at `kids[i]` and the index of its closing tag.
fn open_tag(kids: &[&AstNode<'_>], i: usize) -> Option<(&'static str, usize)> {
    let NodeValue::HtmlInline(h) = &kids[i].data.borrow().value else {
        return None;
    };
    match rawhtml::classify(h) {
        rawhtml::Raw::Open(tag) if tag != "br" => Some((tag, rawhtml::closing(kids, i, tag)?)),
        _ => None,
    }
}

/// When a heading ends in a valid `{#id}`: the index of its last child, a text
/// node, and that node's text without the id.
fn heading_text_without_id<'a>(
    node: &'a AstNode<'a>,
    kids: &[&'a AstNode<'a>],
) -> Option<(usize, String)> {
    let (_, id) = heading_parts(&plain_text(node));
    let id = id?;
    let last = kids.len().checked_sub(1)?;
    let NodeValue::Text(text) = &kids[last].data.borrow().value else {
        return None;
    };
    let visible = text.trim_end().strip_suffix(&format!("{{#{id}}}"))?;
    let visible = if last == 0 {
        visible.trim()
    } else {
        visible.trim_end()
    };
    Some((last, visible.to_string()))
}

/// Parse Markdown the way both renderers read it: comrak, then the rewrites
/// comrak has no option for.
fn parse<'a>(arena: &'a Arena<'a>, markdown: &str) -> &'a AstNode<'a> {
    let root = parse_document(arena, markdown, &build_options());
    normalize(arena, root);
    root
}

/// - `^sup^` and `~sub~` around whitespace go back to text: pandoc's rule,
///   so `2^10 and 3^4` and `~5 to ~10` read as written.
/// - An Outline attachment link (`/api/files.get`, `/api/attachments.redirect`)
///   loses the byte count Outline appends to its label.
fn normalize<'a>(arena: &'a Arena<'a>, root: &'a AstNode<'a>) {
    let text = |s: &str| arena.alloc(AstNode::from(NodeValue::Text(s.to_string().into())));
    for node in root.descendants().collect::<Vec<_>>() {
        let marker = match &node.data.borrow().value {
            NodeValue::Superscript => "^",
            NodeValue::Subscript => "~",
            NodeValue::Link(link)
                if link.url.contains("/api/files.get")
                    || link.url.contains("/api/attachments.redirect") =>
            {
                drop_attachment_size(node);
                continue;
            }
            _ => continue,
        };
        if !plain_text(node).contains(char::is_whitespace) {
            continue;
        }
        node.insert_before(text(marker));
        for child in node.children().collect::<Vec<_>>() {
            node.insert_before(child);
        }
        node.insert_before(text(marker));
        node.detach();
    }
}

fn drop_attachment_size<'a>(link: &'a AstNode<'a>) {
    let Some(last) = link.last_child() else {
        return;
    };
    let mut data = last.data.borrow_mut();
    if let NodeValue::Text(t) = &mut data.value {
        if let Some((label, size)) = t.rsplit_once(' ') {
            if !size.is_empty()
                && size.bytes().all(|b| b.is_ascii_digit())
                && !label.trim().is_empty()
            {
                *t = label.trim_end().to_string().into();
            }
        }
    }
}

/// `image-width`, when it is a length both outputs can use: a percentage of
/// the text width, or `pt`/`mm`/`cm`/`in`.
fn image_width(fm: &frontmatter::Frontmatter) -> Option<String> {
    let raw = fm.text("image-width")?;
    let raw = raw.trim();
    let unit = ["%", "pt", "mm", "cm", "in"]
        .into_iter()
        .find(|u| raw.ends_with(u))?;
    let number = &raw[..raw.len() - unit.len()];
    let valid = !number.is_empty()
        && number.parse::<f64>().is_ok_and(|n| n > 0.0)
        && number.chars().all(|c| c.is_ascii_digit() || c == '.');
    valid.then(|| format!("{number}{unit}"))
}

/// A text run cut at the byline markers `preprocess` leaves: (is a byline,
/// text) pairs.
fn split_bylines(s: &str) -> Vec<(bool, &str)> {
    use preprocess::{BYLINE_CLOSE, BYLINE_OPEN};
    let mut parts = Vec::new();
    let mut rest = s;
    while let Some(open) = rest.find(BYLINE_OPEN) {
        let Some(len) = rest[open..].find(BYLINE_CLOSE) else {
            break;
        };
        parts.push((false, &rest[..open]));
        parts.push((true, &rest[open + BYLINE_OPEN.len_utf8()..open + len]));
        rest = &rest[open + len + BYLINE_CLOSE.len_utf8()..];
    }
    parts.push((false, rest));
    parts.retain(|(_, p)| !p.is_empty());
    parts
}

/// The language a code fence names: its first word, without pandoc's
/// `{.lang}` braces or HedgeDoc's line-number flags (`js=`, `js=101`, `js=+`,
/// `js!`). Attributes after it (`title="x"`) are not part of the language.
fn code_language(info: &str) -> String {
    let word = info.split_whitespace().next().unwrap_or("");
    let word = word.trim_start_matches('{').trim_end_matches('}');
    let word = word.trim_start_matches('.');
    let word = word.split('=').next().unwrap_or("");
    word.trim_end_matches('!').to_string()
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
    // HedgeDoc's `++inserted++` and pandoc's `^[inline note]`.
    e.insert = true;
    e.inline_footnotes = true;
    e.footnotes = true;
    e.autolink = true;
    e.math_dollars = true;
    e.math_code = true;
    // `:smile:` -> unicode emoji; the bundled NotoColorEmoji font renders it.
    e.shortcodes = true;
    o
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
                        let chars: Vec<char> = e.as_str().chars().collect();
                        if let Some((_, cp)) = match_emoji(&chars, 0) {
                            set.insert(cp);
                        }
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

/// Parse a `=200x200`, `=200x`, `=x200` or `200x200` dimension spec. A
/// height alone needs the `=`, so a title like `x5` stays a title.
fn parse_dims(raw: &str) -> Option<(Option<String>, Option<String>)> {
    let raw = raw.trim();
    let s = raw.trim_start_matches('=');
    let (w, h) = s.split_once(['x', 'X'])?;
    let num = |n: &str| -> Option<Option<String>> {
        if n.is_empty() {
            Some(None)
        } else if n.chars().all(|c| c.is_ascii_digit() || c == '.') {
            Some(Some(n.to_string()))
        } else {
            None
        }
    };
    let (width, height) = (num(w)?, num(h)?);
    if width.is_none() && (height.is_none() || !raw.starts_with('=')) {
        return None;
    }
    Some((width, height))
}

/// The file a local image target names, or `None` when it names nothing a host
/// can load: an empty target, a `data:` URI, or any other scheme. `%XX`
/// escapes are decoded, so `my%20pic.png` is the file `my pic.png`.
fn local_image_path(url: &str) -> Option<String> {
    let url = url.trim();
    let scheme = url.split_once(':').map(|(s, _)| s);
    let has_scheme = scheme.is_some_and(|s| {
        s.len() > 1
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    if url.is_empty() || has_scheme {
        return None;
    }
    Some(percent_decode(url))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// What a body image loads: the remote URL itself, or the local file.
fn image_target(url: &str) -> Option<String> {
    if is_remote(url) {
        Some(url.trim().to_string())
    } else {
        local_image_path(url)
    }
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

/// The image a header, footer or cover field shows: `![](path =WxH)`, also
/// wrapped in a link as `[![](path)](url)`.
fn frontmatter_image_path(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(inner) = value.strip_prefix('[').filter(|v| v.starts_with("![")) {
        if let Some(end) = inner.rfind(")](") {
            return frontmatter_image_path(&inner[..=end]);
        }
    }
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

/// What a document asks its host for, gathered in one walk of the document
/// and of the admonition, spoiler and row bodies lifted out of it.
struct Discovery {
    /// Every image target, deduplicated, with any `=WxH` size hint stripped:
    /// remote URLs and local paths, including the frontmatter fields the PDF
    /// template renders as images.
    images: Vec<String>,
    math: bool,
    /// Every ```` ```mermaid ```` fence, deduplicated.
    mermaid: Vec<String>,
}

impl Discovery {
    /// Local images: the HTML output embeds them, so the host supplies the
    /// bytes.
    fn local_images(&self) -> impl Iterator<Item = &str> {
        self.images
            .iter()
            .map(String::as_str)
            .filter(|u| !is_remote(u))
    }

    /// Remote image URLs paired with the `<dir>/<hash>` alias the host
    /// prefetches them to — Typst's sandbox cannot fetch them itself.
    fn remotes(&self, remote_dir: &str) -> Vec<(String, String)> {
        self.images
            .iter()
            .filter(|u| is_remote(u))
            .map(|u| (u.clone(), format!("{remote_dir}/{}", hash_url(u))))
            .collect()
    }
}

/// Parsed rather than string-scanned, so a URL or a fence that only appears
/// as an example inside a code span is not mistaken for the real thing.
fn discover(src: &str) -> Discovery {
    let mut found = Discovery {
        images: Vec::new(),
        math: false,
        mermaid: Vec::new(),
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut add = |found: &mut Discovery, path: String| {
        if !path.is_empty() && seen.insert(path.clone()) {
            found.images.push(path);
        }
    };

    let fm = frontmatter::Frontmatter::parse(src);
    if let Some(value) = fm.text("cover-image") {
        add(
            &mut found,
            frontmatter_image_path(&value).unwrap_or_else(|| value.trim().to_string()),
        );
    }
    for key in [
        "cover-logo",
        "header-left",
        "header-center",
        "header-right",
        "footer-left",
        "footer-center",
        "footer-right",
    ] {
        if let Some(path) = fm.text(key).as_deref().and_then(frontmatter_image_path) {
            add(&mut found, path);
        }
    }

    let mut diagrams = HashSet::new();
    walk_document(src, &mut |value| match value {
        NodeValue::Image(link) => {
            let (url, _) = html::split_dims(&link.url, &link.title);
            if let Some(path) = image_target(&url) {
                add(&mut found, path);
            }
        }
        NodeValue::Math(_) => found.math = true,
        NodeValue::CodeBlock(cb) if cb.info.trim().eq_ignore_ascii_case("mermaid") => {
            let code = cb.literal.strip_suffix('\n').unwrap_or(&cb.literal);
            if diagrams.insert(code.to_string()) {
                found.mermaid.push(code.to_string());
            }
        }
        _ => {}
    });
    found
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
    let root = parse(&arena, &pre.markdown);
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

/// The body of the stand-in definition a parse gets for a footnote defined in
/// another block of the document.
const NOTE_STUB: &str = "\u{E003}";

/// Every footnote definition in the document, top level and lifted bodies
/// alike, as Markdown keyed by name. Each block is parsed on its own, so a
/// reference in an admonition to a note defined outside it would otherwise
/// find nothing. The first definition of a name wins, as in comrak.
fn footnote_sources(src: &str) -> HashMap<String, String> {
    let mut notes = HashMap::new();
    if src.contains("[^") {
        collect_footnote_sources(src, &mut notes);
    }
    notes
}

fn collect_footnote_sources(src: &str, notes: &mut HashMap<String, String>) {
    let pre = preprocess(src);
    // comrak drops a definition nothing in its own parse refers to, so every
    // name that looks defined gets a reference appended below the text.
    let mut markdown = pre.markdown.clone();
    for line in pre.markdown.lines() {
        let name = line
            .trim_start()
            .strip_prefix("[^")
            .and_then(|rest| rest.split_once("]:"))
            .map(|(name, _)| name);
        if let Some(name) = name {
            markdown.push_str(&format!("\n\n[^{name}]"));
        }
    }
    let arena = Arena::new();
    let root = parse(&arena, &markdown);
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut nested = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::FootnoteDefinition(def) => {
                let (start, end) = (data.sourcepos.start.line, data.sourcepos.end.line);
                let mut body: Vec<&str> = lines[start - 1..end.min(lines.len())].to_vec();
                if let Some(first) = body.first_mut() {
                    *first = first
                        .split_once("]:")
                        .map_or("", |(_, rest)| rest)
                        .trim_start();
                }
                for line in body.iter_mut().skip(1) {
                    let indent = line.len() - line.trim_start_matches(' ').len();
                    *line = &line[indent.min(4)..];
                }
                // comrak names inline notes `__inline_N` afresh in every
                // parse, so they belong to the block they were written in.
                if !def.name.starts_with("__inline") {
                    notes
                        .entry(def.name.clone())
                        .or_insert_with(|| body.join("\n"));
                }
            }
            NodeValue::HtmlBlock(hb) => {
                if let Some(a) = parse_placeholder(&hb.literal, "admonition")
                    .and_then(|id| pre.admonitions.get(id))
                {
                    nested.push(a.source.clone());
                } else if let Some(s) =
                    parse_placeholder(&hb.literal, "spoiler").and_then(|id| pre.spoilers.get(id))
                {
                    nested.push(s.source.clone());
                }
            }
            _ => {}
        }
    }
    for source in nested {
        collect_footnote_sources(&source, notes);
    }
}

/// `markdown` plus a stand-in definition for every document footnote it does
/// not define itself, so that comrak turns a reference to one into a
/// reference rather than literal `[^name]` text.
fn with_note_stubs(markdown: &str, notes: &HashMap<String, String>) -> String {
    let mut out = markdown.to_string();
    for name in notes.keys() {
        if !markdown.contains(&format!("[^{name}]:")) {
            out.push_str(&format!("\n\n[^{name}]: {NOTE_STUB}"));
        }
    }
    out
}

fn is_note_stub<'a>(def: &'a AstNode<'a>) -> bool {
    plain_text(def).trim() == NOTE_STUB
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

/// Whether a task-list item is checked: by its own `[x]`, which comrak puts
/// on the item itself or on a `TaskItem` wrapping its body — never by a
/// checked item nested further down.
fn task_checked<'a>(item: &'a AstNode<'a>) -> bool {
    std::iter::once(item).chain(item.children()).any(
        |node| matches!(&node.data.borrow().value, NodeValue::TaskItem(t) if t.symbol.is_some()),
    )
}
