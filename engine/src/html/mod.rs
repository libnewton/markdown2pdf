//! Markdown -> self-contained HTML.
//!
//! The second renderer in the engine. It walks the *same* comrak AST and the
//! same pre-parse passes as the Typst renderer in `typst.rs`, so a syntax
//! feature cannot exist in one output and not the other. Everything the host
//! cannot be asked to do lives here: styling, the table of contents, math,
//! syntax highlighting and asset embedding.
//!
//! Page-only features (cover, DIN letter mode, running header/footer, page
//! numbers) have no HTML counterpart and are deliberately dropped.

mod assets;
mod css;
mod highlight;
pub(crate) mod math;
pub(crate) mod tokens;

use crate::frontmatter::Frontmatter;
use crate::preprocess::{
    parse_placeholder, preprocess, preprocess_citations, split_inline_bibliography, Admonition,
    ListStyle, Preprocessed, Spoiler, CITATION_CLOSE_TOKEN, CITATION_OPEN_TOKEN,
};
use crate::{
    hash_url, heading_parts, is_remote, leading_h1_index, match_emoji, parse, parse_dims,
    plain_text, safe_url, task_checked, unique_heading_id,
};
use assets::Assets;
use comrak::nodes::{AstNode, ListType, NodeValue, TableAlignment};
use comrak::Arena;
use math::Math;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

/// Stand-in for the outline, substituted once every heading is known.
const TOC_SLOT: &str = "\u{e010}md2pdf-toc\u{e011}";

/// Split a HackMD `=WxH` size hint off an image target. It rides either in the
/// title field (`![a](p "=200x120")`, also after a title) or at the end of an
/// angle-bracketed URL (`![a](<p =200x120>)`).
type Dims = (Option<String>, Option<String>);
pub(crate) fn split_dims(url: &str, title: &str) -> (String, Option<Dims>) {
    let mut url = url.trim().to_string();
    // Outline appends the size to a real title: `"My caption =300x200"`.
    let mut dims = parse_dims(title).or_else(|| {
        let (_, size) = title.trim().rsplit_once(' ')?;
        parse_dims(size).filter(|_| size.starts_with('='))
    });
    if dims.is_none() {
        if let Some(sp) = url.rfind(char::is_whitespace) {
            let (head, tail) = url.split_at(sp);
            if let Some(d) = parse_dims(tail.trim()) {
                dims = Some(d);
                url = head.trim().to_string();
            }
        }
    }
    (url, dims)
}

/// The shared design tokens, for `tokens()` in `lib.rs`.
pub(crate) fn tokens_toml() -> String {
    tokens::as_toml()
}

/// Asset key for one Mermaid diagram. Keyed by a hash of the source rather
/// than by position, so host and engine cannot drift out of step.
pub(crate) fn mermaid_key(code: &str) -> String {
    format!("mermaid/{}.svg", hash_url(code))
}

// ==========================================================================
// Document assembly
// ==========================================================================

pub(crate) fn render(src: &str, options: &str, manifest: &str, blob: &[u8]) -> String {
    let flag = |key| option(options, key).is_some_and(|v| v != "0" && !v.is_empty());
    let standalone = flag("standalone");
    let theme = match option(options, "theme") {
        Some("light") => Some("light"),
        Some("dark") => Some("dark"),
        _ => None,
    };
    // The render is backed by a source the reader can edit: blocks carry the
    // line they came from, and task checkboxes are live. A download has no
    // source to point at, so it never sets this.
    let editable = flag("editable");
    let remote_dir = option(options, "remote-dir")
        .filter(|d| !d.is_empty())
        .unwrap_or(crate::DEFAULT_REMOTE_DIR);
    let fm = Frontmatter::parse(src);
    let inline_bib = fm
        .text("bibliography")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("inline"));
    let (body_src, bib_src) = if inline_bib {
        split_inline_bibliography(src)
    } else {
        (src.to_string(), String::new())
    };

    let mut doc = Doc {
        assets: Assets::decode(manifest, blob),
        math: Math::new(),
        german: fm.text("lang").is_some_and(|l| l.starts_with("de")),
        citations: !bib_src.is_empty(),
        cite_keys: bibtex_keys(&bib_src),
        cites: Vec::new(),
        notes: Vec::new(),
        note_index: HashMap::new(),
        note_refs: HashMap::new(),
        note_sources: crate::footnote_sources(&body_src),
        headings: Vec::new(),
        slugs: HashMap::new(),
        editable,
        remote_dir: remote_dir.to_string(),
        bibliography_title: None,
        title_h1: None,
        image_width: crate::image_width(&fm).unwrap_or_else(|| "100%".to_string()),
        soft_breaks: fm.get("breaks").is_some_and(crate::frontmatter::is_off),
    };
    doc.bibliography_title = match fm.get("bibliography-title") {
        None => Some(tokens::string("references", doc.german).to_string()),
        Some(v) if crate::frontmatter::is_off(v) => None,
        Some(v) => v.text().filter(|t| !t.trim().is_empty()),
    };

    // Title precedence mirrors `lib.typ`: frontmatter beats a leading H1, and
    // the H1 is dropped only when it is the thing that became the title. A
    // frontmatter title does not consume it — that heading belongs to the body
    // and nothing else would take its place.
    let fm_title = fm.text("title").filter(|t| !t.is_empty());

    // The top level is its own baseline: line N is line N.
    let base: Vec<u32> = (1..=body_src.lines().count() as u32 + 1).collect();
    let body = render_source(&body_src, &mut doc, fm_title.is_none(), &base);
    let title = fm_title.or_else(|| doc.title_h1.take());

    let mut main = String::new();
    main.push_str(&title_block(&fm, title.as_deref(), &doc));
    main.push_str("<div class=\"md2pdf-body\">");
    main.push_str(&body);
    main.push_str("</div>");
    main.push_str(&doc.footnote_section());
    main.push_str(&bibliography(&bib_src, &doc));

    let outline = if fm.flag("hide-toc-button") {
        String::new()
    } else {
        toc(&doc.headings, doc.german)
    };
    let theme_toggle = if standalone && !fm.flag("hide-theme-toggle") {
        theme_toggle(doc.german)
    } else {
        String::new()
    };
    let main = main.replace(TOC_SLOT, &inline_toc(&doc.headings));

    let lang = fm
        .text("lang")
        .unwrap_or_else(|| if doc.german { "de" } else { "en" }.to_string());
    // Only the standalone export carries the behaviour script. A fragment is
    // mounted by a host that has to re-execute it to make it run at all, and
    // "take the first <script> out of the document and run it" is not a
    // primitive worth handing to a renderer whose input is untrusted — the
    // editor implements copy and anchor scrolling itself instead.
    let script = if standalone {
        format!("<script>{}</script>", css::SCRIPT)
    } else {
        String::new()
    };
    let fragment = format!(
        "<style>{fonts}{style}</style>\
         <div class=\"md2pdf\" id=\"md2pdf-root\" lang=\"{lang}\">\
         {outline}<main class=\"md2pdf-doc\">{main}</main>{theme_toggle}</div>{script}",
        fonts = math_font_faces(&doc, &main),
        style = css::style(),
        lang = esc_attr(&lang),
    );
    if standalone {
        document(
            &fragment,
            &esc_attr(&lang),
            title.as_deref().unwrap_or("Document"),
            theme,
        )
    } else {
        fragment
    }
}

/// `@font-face` rules for the math font, embedded as `data:` URIs.
///
/// Only the standalone export asks its host for the font bytes: the preview
/// pane gets the same faces from the app's own stylesheet, because a
/// `@font-face` inside a shadow root is ignored by Chromium anyway — and a few
/// hundred kB of base64 has no business on the per-keystroke render path.
///
/// The math alphanumerics (`\mathbb`, `\mathcal`, `\mathfrak`, …) live in a
/// second file: they are half the font's weight and most documents never use
/// one, so that face ships only when a character from the block is on the page.
fn math_font_faces(doc: &Doc, markup: &str) -> String {
    let Some(base) = doc.assets.data_uri("fonts/math.woff2") else {
        return String::new();
    };
    let face = |extra: &str, uri: &str| {
        format!(
            "@font-face{{font-family:\"NewCM Math\";{extra}\
             src:url({uri}) format(\"woff2\");font-display:swap}}"
        )
    };
    let mut out = face("", &base);
    if markup
        .chars()
        .any(|c| ('\u{1D400}'..='\u{1D7FF}').contains(&c))
    {
        if let Some(alpha) = doc.assets.data_uri("fonts/math-alpha.woff2") {
            out.push_str(&face("unicode-range:U+1D400-1D7FF;", &alpha));
        }
    }
    out
}

fn document(fragment: &str, lang: &str, title: &str, theme: Option<&str>) -> String {
    let theme = theme
        .map(|t| format!(" data-theme=\"{t}\""))
        .unwrap_or_default();
    format!(
        "<!doctype html>\n<html lang=\"{lang}\"{theme}>\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"generator\" content=\"md2pdf\">\n\
         <title>{title}</title>\n\
         <style>html,body{{margin:0;padding:0;background:var(--md-bg);}}</style>\n\
         </head>\n<body>{fragment}</body>\n</html>\n",
        title = esc_text(title)
    )
}

fn theme_toggle(german: bool) -> String {
    let label = tokens::string("theme-toggle", german);
    format!(
        "<button class=\"md2pdf-theme-toggle\" type=\"button\" aria-label=\"{label}\" \
         title=\"{label}\"><span class=\"md2pdf-theme-moon\" aria-hidden=\"true\"></span>\
         <span class=\"md2pdf-theme-sun\" aria-hidden=\"true\"></span>\
         <span class=\"md2pdf-sr\">{label}</span></button>"
    )
}

fn title_block(fm: &Frontmatter, title: Option<&str>, doc: &Doc) -> String {
    let subtitle = fm.text("subtitle").unwrap_or_default();
    let authors = fm.list("authors");
    let authors = if authors.is_empty() {
        fm.list("author")
    } else {
        authors
    };
    let date = fm.text("date").unwrap_or_default();
    if title.is_none_or(str::is_empty)
        && subtitle.is_empty()
        && authors.is_empty()
        && date.is_empty()
    {
        return String::new();
    }
    let mut out = String::from("<header class=\"md2pdf-titleblock\">");
    if let Some(t) = title.filter(|t| !t.is_empty()) {
        out.push_str(&format!("<h1>{}</h1>", inline_text(doc, t)));
    }
    if !subtitle.is_empty() {
        out.push_str(&format!(
            "<p class=\"md2pdf-subtitle\">{}</p>",
            inline_text(doc, &subtitle)
        ));
    }
    let mut byline: Vec<String> = authors.iter().map(|a| inline_text(doc, a)).collect();
    if !date.is_empty() {
        byline.push(esc_text(&date));
    }
    if !byline.is_empty() {
        out.push_str("<p class=\"md2pdf-byline\">");
        for part in byline {
            out.push_str(&format!("<span>{part}</span>"));
        }
        out.push_str("</p>");
    }
    out.push_str("</header>");
    out
}

/// The drawer outline plus the controls that open it. Pure CSS: a checkbox
/// drives the transform, so opening it needs no script. The button itself is
/// icon-only — the label rides along for screen readers.
fn toc(headings: &[Heading], german: bool) -> String {
    if headings.len() < 2 {
        return String::new();
    }
    let label = tokens::string("contents", german);
    format!(
        "<input type=\"checkbox\" class=\"md2pdf-toc-state\" id=\"md2pdf-toc-state\" \
           aria-label=\"{label}\">\
         <label class=\"md2pdf-toc-btn\" for=\"md2pdf-toc-state\">\
           <span class=\"md2pdf-sr\">{label}</span></label>\
         <label class=\"md2pdf-toc-scrim\" for=\"md2pdf-toc-state\" aria-hidden=\"true\"></label>\
         <nav class=\"md2pdf-toc\" aria-label=\"{label}\">\
           <p class=\"md2pdf-toc-title\">{label}</p>{}</nav>",
        toc_list(headings)
    )
}

fn inline_toc(headings: &[Heading]) -> String {
    if headings.is_empty() {
        return String::new();
    }
    format!(
        "<nav class=\"md2pdf-toc-inline\">{}</nav>",
        toc_list(headings)
    )
}

fn toc_list(headings: &[Heading]) -> String {
    let base = headings.iter().map(|h| h.level).min().unwrap_or(1);
    let mut out = String::from("<ol>");
    for h in headings {
        out.push_str(&format!(
            "<li data-level=\"{}\"><a href=\"#{}\">{}</a></li>",
            (h.level - base).min(6),
            esc_attr(&h.id),
            esc_text(&h.text)
        ));
    }
    out.push_str("</ol>");
    out
}

/// Read one `key=value` line out of the options blob.
fn option<'a>(options: &'a str, key: &str) -> Option<&'a str> {
    options
        .lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim())
}

// ==========================================================================
// Document-wide state
// ==========================================================================

struct Heading {
    level: u8,
    id: String,
    text: String,
}

struct Note {
    number: usize,
    /// Id of the first reference, so the note can link back to it.
    back: String,
    html: String,
}

struct Doc<'b> {
    assets: Assets<'b>,
    math: Math,
    german: bool,
    citations: bool,
    /// The keys of the inline bibliography; only these are cited.
    cite_keys: HashSet<String>,
    /// Citation keys in order of first use — that order is the numbering.
    cites: Vec<String>,
    notes: Vec<Note>,
    note_index: HashMap<String, usize>,
    /// How often each note number has been referenced so far.
    note_refs: HashMap<usize, usize>,
    /// Every footnote definition in the document, by name.
    note_sources: HashMap<String, String>,
    headings: Vec<Heading>,
    slugs: HashMap<String, usize>,
    editable: bool,
    /// Directory prefix of the `<dir>/<hash>` keys of remote images.
    remote_dir: String,
    /// The leading H1's text, once the top-level render promoted it to title.
    title_h1: Option<String>,
    /// The reference list's heading, `None` for none at all.
    bibliography_title: Option<String>,
    /// `image-width`: the width of an image without a size of its own.
    image_width: String,
    /// `breaks: false`: a source line break is a space, not a line break.
    soft_breaks: bool,
}

impl Doc<'_> {
    fn slug(&mut self, text: &str, custom: Option<&str>) -> String {
        unique_heading_id(&mut self.slugs, text, custom)
    }

    fn cite_number(&mut self, key: &str) -> usize {
        if let Some(i) = self.cites.iter().position(|k| k == key) {
            return i + 1;
        }
        self.cites.push(key.to_string());
        self.cites.len()
    }

    fn footnote_section(&self) -> String {
        if self.notes.is_empty() {
            return String::new();
        }
        let title = tokens::string("notes", self.german);
        let mut out = format!("<section class=\"md2pdf-notes\"><h2>{title}</h2><ol>");
        for note in &self.notes {
            let back = format!(
                "<a class=\"md2pdf-backref\" href=\"#{}\" aria-label=\"back to reference\">\u{21a9}</a>",
                esc_attr(&note.back)
            );
            // Tuck the backlink inside the closing paragraph so it trails the
            // note text instead of dropping onto a line of its own.
            let html = match note.html.strip_suffix("</p>") {
                Some(head) => format!("{head}{back}</p>"),
                None => format!("{}{back}", note.html),
            };
            out.push_str(&format!("<li id=\"md2pdf-fn-{}\">{html}</li>", note.number));
        }
        out.push_str("</ol></section>");
        out
    }
}

// ==========================================================================
// Per-source frame — one parse of one Markdown string
// ==========================================================================

struct Frame<'a> {
    notes: HashMap<String, &'a AstNode<'a>>,
    admonitions: Vec<Admonition>,
    spoilers: Vec<Spoiler>,
    table_widths: Vec<Vec<usize>>,
    pending_widths: Cell<Option<usize>>,
    list_styles: Vec<ListStyle>,
    /// Set by a `liststyle` placeholder, consumed by the list right after it.
    pending_style: Cell<Option<ListStyle>>,
    /// Original source line per line of the text comrak was handed, already
    /// rebased through any enclosing block.
    origin: Vec<u32>,
}

impl<'a> Frame<'a> {
    fn new(pre: Preprocessed) -> Self {
        Self {
            notes: HashMap::new(),
            admonitions: pre.admonitions,
            spoilers: pre.spoilers,
            table_widths: pre.table_widths,
            pending_widths: Cell::new(None),
            list_styles: pre.list_styles,
            pending_style: Cell::new(None),
            origin: pre.origin,
        }
    }

    /// The source line a node came from, or 0 when it came from a line some
    /// pass invented.
    fn line_of(&self, node: &AstNode) -> u32 {
        let n = node.data.borrow().sourcepos.start.line;
        self.origin.get(n.saturating_sub(1)).copied().unwrap_or(0)
    }

    fn collect_notes(&mut self, node: &'a AstNode<'a>) {
        if let NodeValue::FootnoteDefinition(def) = &node.data.borrow().value {
            self.notes.insert(def.name.clone(), node);
        }
        for child in node.children() {
            self.collect_notes(child);
        }
    }
}

/// Render one Markdown source. Recursive: admonition, spoiler and row bodies
/// come back through here, sharing `doc` so footnote numbers, heading ids and
/// citations stay consistent across the whole document.
/// Render a document, or a block's extracted body.
///
/// `base` maps this source's lines onto the original document's, so a node
/// inside an admonition inside a blank-line run still reports the line the
/// author typed. It is applied once here rather than at every use.
fn render_source(src: &str, doc: &mut Doc, strip_h1: bool, base: &[u32]) -> String {
    let prepared = if doc.citations {
        preprocess_citations(src, &doc.cite_keys).0
    } else {
        src.to_string()
    };
    // Citations are the one pass that rewrites within a line rather than
    // across lines, so it leaves the mapping alone.
    let pre = preprocess_rebased(&prepared, base);
    let arena = Arena::new();
    let markdown = crate::with_note_stubs(&pre.markdown, &doc.note_sources);
    let root = parse(&arena, &markdown);
    let mut frame = Frame::new(pre);
    frame.collect_notes(root);

    let children: Vec<&AstNode> = root.children().collect();
    // A leading H1 with text becomes the title and leaves the body.
    let skip = if strip_h1 {
        leading_h1_index(&children).filter(|&i| {
            let text = heading_parts(&plain_text(children[i])).0;
            let promoted = !text.is_empty();
            if promoted {
                doc.title_h1 = Some(text);
            }
            promoted
        })
    } else {
        None
    };
    children
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != skip)
        .map(|(_, c)| {
            let html = block(doc, &frame, c);
            if doc.editable {
                with_line(html, frame.line_of(c))
            } else {
                html
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("")
}

/// Run the pre-parse passes over a block's source and map every origin they
/// record — its own lines and those of the blocks it lifted out — into the
/// original document's coordinates.
fn preprocess_rebased(src: &str, base: &[u32]) -> Preprocessed {
    let mut pre = preprocess(src);
    pre.origin = crate::preprocess::rebase(&pre.origin, base);
    for a in &mut pre.admonitions {
        a.origin = crate::preprocess::rebase(&a.origin, base);
    }
    for s in &mut pre.spoilers {
        s.origin = crate::preprocess::rebase(&s.origin, base);
    }
    pre
}

/// Tag a rendered block with the source line it came from.
///
/// Written into the opening tag rather than around it, so the attribute costs
/// no element and cannot disturb the layout it is describing.
fn with_line(html: String, line: u32) -> String {
    if line == 0 || !html.starts_with('<') {
        return html;
    }
    match html.find([' ', '>']) {
        Some(at) => format!("{} data-md-line=\"{line}\"{}", &html[..at], &html[at..]),
        None => html,
    }
}

// ==========================================================================
// Block level
// ==========================================================================

fn blocks<'a>(doc: &mut Doc, f: &Frame<'a>, node: &'a AstNode<'a>) -> String {
    node.children()
        .map(|c| block(doc, f, c))
        .filter(|s| !s.is_empty())
        .collect()
}

fn block<'a>(doc: &mut Doc, f: &Frame<'a>, node: &'a AstNode<'a>) -> String {
    let data = node.data.borrow();
    let value = &data.value;
    // A pending width id belongs to the table right after its placeholder.
    if !matches!(value, NodeValue::Table(_) | NodeValue::HtmlBlock(_)) {
        f.pending_widths.set(None);
    }
    if !matches!(value, NodeValue::List(_) | NodeValue::HtmlBlock(_)) {
        f.pending_style.set(None);
    }
    match value {
        NodeValue::FootnoteDefinition(_) => String::new(),
        NodeValue::Document => blocks(doc, f, node),
        NodeValue::Heading(h) => heading(doc, f, node, h.level.clamp(1, 6)),
        NodeValue::Paragraph => paragraph(doc, f, node),
        NodeValue::ThematicBreak(_) => "<hr>".to_string(),
        NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => {
            format!("<blockquote>{}</blockquote>", blocks(doc, f, node))
        }
        NodeValue::List(_) => list(doc, f, node),
        NodeValue::Item(_) | NodeValue::TaskItem(_) => blocks(doc, f, node),
        NodeValue::CodeBlock(cb) => code_block(doc, &cb.info, &cb.literal),
        NodeValue::HtmlBlock(hb) => {
            if let Some(id) = parse_placeholder(&hb.literal, "admonition") {
                admonition(doc, f, id)
            } else if let Some(id) = parse_placeholder(&hb.literal, "spoiler") {
                spoiler(doc, f, id)
            } else if let Some(id) = parse_placeholder(&hb.literal, "tablewidths") {
                f.pending_widths.set(Some(id));
                String::new()
            } else if let Some(id) = parse_placeholder(&hb.literal, "liststyle") {
                f.pending_style.set(f.list_styles.get(id).copied());
                String::new()
            } else {
                // Raw HTML in the source is shown, not executed — same stance
                // as the Typst renderer, which escapes it too. Comments go.
                let shown = crate::rawhtml::without_comments(&hb.literal);
                if shown.trim().is_empty() {
                    String::new()
                } else {
                    format!("<p>{}</p>", esc_text(shown.trim()))
                }
            }
        }
        NodeValue::Table(_) => table(doc, f, node),
        NodeValue::Math(m) => doc.math.render(m.display_math, &m.literal),
        _ => format!("<p>{}</p>", inlines(doc, f, node)),
    }
}

fn heading<'a>(doc: &mut Doc, f: &Frame<'a>, node: &'a AstNode<'a>, level: u8) -> String {
    let (text, custom) = heading_parts(&plain_text(node));
    let id = doc.slug(&text, custom.as_deref());
    let body = heading_inlines(doc, f, node);
    doc.headings.push(Heading {
        level,
        id: id.clone(),
        text,
    });
    format!(
        "<h{level} id=\"{id}\"><a class=\"md2pdf-anchor\" href=\"#{id}\" aria-hidden=\"true\" \
         tabindex=\"-1\">#</a>{body}</h{level}>",
        id = esc_attr(&id)
    )
}

fn heading_inlines<'a>(doc: &mut Doc, f: &Frame<'a>, node: &'a AstNode<'a>) -> String {
    let kids: Vec<&AstNode> = node.children().collect();
    if let Some((last, visible)) = crate::heading_text_without_id(node, &kids) {
        return nodes(doc, f, &kids[..last]) + &text_run(doc, &visible);
    }
    nodes(doc, f, &kids)
}

fn paragraph<'a>(doc: &mut Doc, f: &Frame<'a>, node: &'a AstNode<'a>) -> String {
    let plain = plain_text(node);
    match plain.trim().to_ascii_lowercase().as_str() {
        "[toc]" => return TOC_SLOT.to_string(),
        "[[pagebreak]]" => return "<hr class=\"md2pdf-pagebreak\">".to_string(),
        "[[md2pdf-blank-line]]" => return "<div class=\"md2pdf-spacer\"></div>".to_string(),
        _ => {}
    }
    let body = inlines(doc, f, node);
    // A paragraph holding nothing but a figure must not nest inside a <p>.
    if body.starts_with("<figure") || body.starts_with("<div class=\"md2pdf-mermaid\"") {
        body
    } else {
        format!("<p>{body}</p>")
    }
}

fn admonition(doc: &mut Doc, f: &Frame<'_>, id: usize) -> String {
    let Some(a) = f.admonitions.get(id) else {
        return String::new();
    };
    let (kind, title, source) = (a.kind.clone(), a.title.clone(), a.source.clone());
    let origin = a.origin.clone();
    match kind.as_str() {
        "left" | "center" | "right" => {
            let inner = render_source(&source, doc, false, &origin);
            if inner.trim().is_empty() {
                String::new()
            } else {
                format!("<div class=\"md2pdf-{kind}\">{inner}</div>")
            }
        }
        "row" => row(doc, &source, &origin),
        _ => {
            let inner = render_source(&source, doc, false, &origin);
            let label = if title.is_empty() {
                tokens::label(&kind, doc.german).to_string()
            } else {
                title
            };
            format!(
                "<aside class=\"md2pdf-adm md2pdf-adm-{kind}\">\
                 <strong class=\"md2pdf-adm-label\">{}</strong>{inner}</aside>",
                esc_text(&label)
            )
        }
    }
}

fn spoiler(doc: &mut Doc, f: &Frame<'_>, id: usize) -> String {
    let Some(s) = f.spoilers.get(id) else {
        return String::new();
    };
    let (summary, source, origin) = (s.summary.clone(), s.source.clone(), s.origin.clone());
    let inner = render_source(&source, doc, false, &origin);
    format!(
        "<details open><summary>{}</summary>{inner}</details>",
        esc_text(&summary)
    )
}

/// `::::row` — every top-level block of the body becomes one grid column.
fn row(doc: &mut Doc, source: &str, base: &[u32]) -> String {
    if source.trim().is_empty() {
        return String::new();
    }
    let pre = preprocess_rebased(source, base);
    let arena = Arena::new();
    let markdown = crate::with_note_stubs(&pre.markdown, &doc.note_sources);
    let root = parse(&arena, &markdown);
    let mut frame = Frame::new(pre);
    frame.collect_notes(root);
    let cells: Vec<String> = root
        .children()
        .map(|c| block(doc, &frame, c))
        .filter(|s| !s.is_empty())
        .map(|c| format!("<div>{c}</div>"))
        .collect();
    if cells.is_empty() {
        return String::new();
    }
    format!(
        "<div class=\"md2pdf-row\" style=\"--md-cols:{}\">{}</div>",
        cells.len(),
        cells.concat()
    )
}

fn code_block(doc: &Doc, info: &str, literal: &str) -> String {
    let code = literal.strip_suffix('\n').unwrap_or(literal);
    let info = info.trim();
    if info.eq_ignore_ascii_case("mermaid") {
        return mermaid(doc, code);
    }
    let lang = crate::code_language(info);
    let lang = lang.as_str();
    let lines: String = highlight::highlight(lang, code)
        .split('\n')
        .map(|l| format!("<span class=\"md2pdf-line\">{l}</span>"))
        .collect();
    let lang_attr = if lang.is_empty() {
        String::new()
    } else {
        format!(" data-lang=\"{}\"", esc_attr(lang))
    };
    let (copy, done) = (
        tokens::string("copy", doc.german),
        tokens::string("copied", doc.german),
    );
    format!(
        "<div class=\"md2pdf-code\"{lang_attr}>\
         <button class=\"md2pdf-copy\" type=\"button\" data-done=\"{done}\">{copy}</button>\
         <pre><code>{lines}</code></pre></div>"
    )
}

/// A diagram, as an image rather than inline SVG.
///
/// The SVG is the mmdr plugin's output over a diagram source the document
/// controls, so it is not ours to trust: inline, one hostile `onload` or
/// `<foreignObject>` would execute, and in the editor's preview it would land
/// in the page rather than the shadow root. Loaded through `<img>` it cannot
/// script or fetch, whatever it contains.
fn mermaid(doc: &Doc, code: &str) -> String {
    let label = tokens::string("mermaid", doc.german);
    match doc.assets.data_uri(&mermaid_key(code)) {
        Some(src) => format!(
            "<figure class=\"md2pdf-mermaid\"><img src=\"{src}\" alt=\"{label}\" \
             loading=\"lazy\" decoding=\"async\"></figure>"
        ),
        None => format!(
            "<div class=\"md2pdf-code\" data-lang=\"mermaid\"><pre><code>{}</code></pre></div>",
            esc_text(code)
        ),
    }
}

fn list<'a>(doc: &mut Doc, f: &Frame<'a>, node: &'a AstNode<'a>) -> String {
    let NodeValue::List(nl) = node.data.borrow().value.clone() else {
        return String::new();
    };
    if nl.is_task_list {
        let items: String = node.children().map(|i| task_item(doc, f, i)).collect();
        return format!("<ul class=\"md2pdf-tasks\">{items}</ul>");
    }
    let items: String = node
        .children()
        .map(|item| format!("<li>{}</li>", item_body(doc, f, item)))
        .collect();
    if nl.list_type == ListType::Ordered {
        let start = if nl.start > 1 {
            format!(" start=\"{}\"", nl.start)
        } else {
            String::new()
        };
        let kind = f
            .pending_style
            .take()
            .map(|style| format!(" type=\"{}\"", style.kind))
            .unwrap_or_default();
        format!("<ol{kind}{start}>{items}</ol>")
    } else {
        format!("<ul>{items}</ul>")
    }
}

/// List-item content, unwrapping a lone paragraph so tight lists stay tight.
fn item_body<'a>(doc: &mut Doc, f: &Frame<'a>, item: &'a AstNode<'a>) -> String {
    let kids: Vec<&'a AstNode<'a>> = item
        .children()
        .flat_map(|c| {
            if matches!(c.data.borrow().value, NodeValue::TaskItem(_)) {
                c.children().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect();
    let single_paragraph = kids
        .first()
        .is_some_and(|c| matches!(c.data.borrow().value, NodeValue::Paragraph));
    kids.iter()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 && single_paragraph {
                inlines(doc, f, c)
            } else {
                block(doc, f, c)
            }
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn task_item<'a>(doc: &mut Doc, f: &Frame<'a>, item: &'a AstNode<'a>) -> String {
    let checked = if task_checked(item) { " checked" } else { "" };
    // comrak marks the whole list as a task list when any one item is a task,
    // so a plain `Item` sibling arrives here too. It has no `[ ]` to write to,
    // so it gets no line and stays inert.
    let is_task = matches!(item.data.borrow().value, NodeValue::TaskItem(_));
    let line = if is_task { f.line_of(item) } else { 0 };

    let (attrs, id) = if doc.editable && line > 0 {
        (
            format!(" data-md-line=\"{line}\" aria-labelledby=\"md2pdf-task-{line}\""),
            format!(" id=\"md2pdf-task-{line}\""),
        )
    } else {
        (" disabled".to_string(), String::new())
    };
    format!(
        "<li class=\"md2pdf-task\"><input type=\"checkbox\"{attrs}{checked}>\
         <div{id}>{}</div></li>",
        item_body(doc, f, item)
    )
}

fn table<'a>(doc: &mut Doc, f: &Frame<'a>, node: &'a AstNode<'a>) -> String {
    let widths = f
        .pending_widths
        .take()
        .and_then(|id| f.table_widths.get(id))
        .cloned();
    let NodeValue::Table(t) = node.data.borrow().value.clone() else {
        return String::new();
    };
    let rows: Vec<&AstNode> = node.children().collect();
    let Some(head) = rows.first() else {
        return String::new();
    };
    let columns = head.children().count().max(1);

    let align_of = |i: usize| match t.alignments.get(i) {
        Some(TableAlignment::Right) => " align=\"right\"",
        Some(TableAlignment::Center) => " align=\"center\"",
        _ => "",
    };
    let cells = |doc: &mut Doc, row: &'a AstNode<'a>, tag: &str| -> String {
        row.children()
            .enumerate()
            .map(|(i, cell)| format!("<{tag}{}>{}</{tag}>", align_of(i), inlines(doc, f, cell)))
            .collect()
    };

    // `---+` column markers are relative weights; percentages carry them over.
    let colgroup = match &widths {
        Some(w) if w.iter().any(|x| *x != 1) => {
            let total: usize = (0..columns).map(|i| w.get(i).copied().unwrap_or(1)).sum();
            let cols: String = (0..columns)
                .map(|i| {
                    let share = w.get(i).copied().unwrap_or(1) as f64 * 100.0 / total.max(1) as f64;
                    format!("<col style=\"width:{share:.4}%\">")
                })
                .collect();
            format!("<colgroup>{cols}</colgroup>")
        }
        _ => String::new(),
    };

    let thead = format!("<thead><tr>{}</tr></thead>", cells(doc, head, "th"));
    let body: String = rows[1..]
        .iter()
        .map(|row| format!("<tr>{}</tr>", cells(doc, row, "td")))
        .collect();
    format!(
        "<div class=\"md2pdf-table\"><table>{colgroup}{thead}<tbody>{body}</tbody></table></div>"
    )
}

// ==========================================================================
// Inline level
// ==========================================================================

fn inlines<'a>(doc: &mut Doc, f: &Frame<'a>, node: &'a AstNode<'a>) -> String {
    nodes(doc, f, &node.children().collect::<Vec<_>>())
}

/// Sibling inline nodes, pairing raw formatting tags as `render_nodes` does
/// in `typst.rs`. The tag is rebuilt from its name, so no attribute survives.
fn nodes<'a>(doc: &mut Doc, f: &Frame<'a>, kids: &[&'a AstNode<'a>]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < kids.len() {
        if let Some((tag, close)) = crate::open_tag(kids, i) {
            let body = nodes(doc, f, &kids[i + 1..close]);
            out.push_str(&format!("<{tag}>{body}</{tag}>"));
            i = close + 1;
            continue;
        }
        out.push_str(&inline(doc, f, kids[i]));
        i += 1;
    }
    out
}

fn inline<'a>(doc: &mut Doc, f: &Frame<'a>, node: &'a AstNode<'a>) -> String {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(t) => text_run(doc, t),
        NodeValue::SoftBreak if doc.soft_breaks => "\n".to_string(),
        NodeValue::SoftBreak => "<br>".to_string(),
        NodeValue::LineBreak => "<br>".to_string(),
        NodeValue::Escaped => inlines(doc, f, node),
        NodeValue::Emph(_) => format!("<em>{}</em>", inlines(doc, f, node)),
        NodeValue::Strong(_) => format!("<strong>{}</strong>", inlines(doc, f, node)),
        NodeValue::Strikethrough(_) => format!("<del>{}</del>", inlines(doc, f, node)),
        NodeValue::Superscript => format!("<sup>{}</sup>", inlines(doc, f, node)),
        NodeValue::Subscript => format!("<sub>{}</sub>", inlines(doc, f, node)),
        NodeValue::Underline => format!("<u>{}</u>", inlines(doc, f, node)),
        NodeValue::Insert => format!("<ins>{}</ins>", inlines(doc, f, node)),
        NodeValue::Code(c) => format!("<code>{}</code>", esc_text(&c.literal)),
        NodeValue::Math(m) => doc.math.render(m.display_math, &m.literal),
        NodeValue::HtmlInline(h) => match crate::rawhtml::classify(h) {
            crate::rawhtml::Raw::Comment => String::new(),
            crate::rawhtml::Raw::Open("br") => "<br>".to_string(),
            _ => esc_text(h),
        },
        NodeValue::ShortCode(s) => emoji_run(doc, &s.emoji),
        NodeValue::Link(l) => {
            let label = inlines(doc, f, node);
            link(&l.url, &label)
        }
        NodeValue::Image(l) => image(doc, &l.url, &l.title, &plain_text(node)),
        NodeValue::FootnoteReference(r) => footnote(doc, f, &r.name),
        _ => inlines(doc, f, node),
    }
}

fn link(url: &str, label: &str) -> String {
    let label = if label.trim().is_empty() {
        esc_text(url)
    } else {
        label.to_string()
    };
    match safe_url(url) {
        Some(href) => {
            let external = href.starts_with("http://") || href.starts_with("https://");
            let rel = if external {
                " rel=\"noopener noreferrer\""
            } else {
                ""
            };
            format!("<a href=\"{}\"{rel}>{label}</a>", esc_attr(href))
        }
        // A rejected scheme still shows its text; it just is not clickable.
        None => format!("<span class=\"md2pdf-missing\">{label}</span>"),
    }
}

fn image(doc: &Doc, url: &str, title: &str, alt: &str) -> String {
    let (url, dims) = split_dims(url, title);
    let key = if is_remote(&url) {
        Some(format!("{}/{}", doc.remote_dir, hash_url(&url)))
    } else {
        crate::local_image_path(&url)
    };

    let caption = alt.trim();
    // An alt that is only a dimension spec is a size hint, not a caption.
    let caption_is_dims =
        caption.starts_with('=') || caption.chars().next().is_some_and(|c| c.is_ascii_digit());
    let caption = if caption_is_dims { "" } else { caption };

    let Some(src) = key.and_then(|key| doc.assets.data_uri(&key)) else {
        let what = if caption.is_empty() {
            url.as_str()
        } else {
            caption
        };
        return format!("<span class=\"md2pdf-missing\">{}</span>", esc_text(what));
    };

    let mut attrs = String::new();
    match &dims {
        Some((w, h)) => {
            if let Some(w) = w {
                attrs.push_str(&format!(" width=\"{}\"", esc_attr(w)));
            }
            if let Some(h) = h {
                attrs.push_str(&format!(" height=\"{}\"", esc_attr(h)));
            }
        }
        // No size given means `image-width`, "as wide as the text" by
        // default, matching the PDF.
        None => attrs.push_str(&format!(" style=\"width:{}\"", doc.image_width)),
    }
    let img = format!(
        "<img src=\"{src}\" alt=\"{}\" loading=\"lazy\" decoding=\"async\"{attrs}>",
        esc_attr(caption)
    );
    if caption.is_empty() {
        format!("<figure>{img}</figure>")
    } else {
        format!(
            "<figure>{img}<figcaption>{}</figcaption></figure>",
            esc_text(caption)
        )
    }
}

fn footnote<'a>(doc: &mut Doc, f: &Frame<'a>, name: &str) -> String {
    let Some(def) = f.notes.get(name).copied() else {
        return String::new();
    };
    // Inline notes are named afresh in every parse; only their node is unique.
    let key = if name.starts_with("__inline") {
        format!("{name}@{:p}", def)
    } else {
        name.to_string()
    };
    let number = match doc.note_index.get(&key) {
        Some(n) => *n,
        None => {
            // Reserve the slot before rendering, so a self-referencing note
            // terminates instead of recursing forever.
            let number = doc.notes.len() + 1;
            doc.note_index.insert(key, number);
            let back = format!("md2pdf-fnref-{number}");
            doc.notes.push(Note {
                number,
                back: back.clone(),
                html: String::new(),
            });
            // A note defined in another block is rendered from its source.
            let html = match doc
                .note_sources
                .get(name)
                .filter(|_| crate::is_note_stub(def))
            {
                Some(source) => render_source(&source.clone(), doc, false, &[]),
                None => blocks(doc, f, def),
            };
            doc.notes[number - 1].html = html;
            number
        }
    };
    // Every reference needs an id of its own; the note links back to the first.
    let seen = doc.note_refs.entry(number).or_insert(0);
    *seen += 1;
    let id = match *seen {
        1 => format!("md2pdf-fnref-{number}"),
        n => format!("md2pdf-fnref-{number}-{n}"),
    };
    format!(
        "<sup class=\"md2pdf-fnref\" id=\"{id}\">\
         <a href=\"#md2pdf-fn-{number}\">[{number}]</a></sup>"
    )
}

/// A text run: bylines, `==mark==` spans, citation tokens and emoji,
/// everything else escaped. Mirrors `render_text` in `typst.rs`.
fn text_run(doc: &mut Doc, s: &str) -> String {
    let mut out = String::new();
    for (byline, part) in crate::split_bylines(s) {
        if byline {
            out.push_str(&format!(
                "<span class=\"md2pdf-tag-byline\">{}</span>",
                emoji_run(doc, part)
            ));
        } else {
            out.push_str(&marks_run(doc, part));
        }
    }
    out
}

fn marks_run(doc: &mut Doc, s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    loop {
        let mark = rest.find("==");
        let citation = doc
            .citations
            .then(|| rest.find(CITATION_OPEN_TOKEN))
            .flatten();
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
                if let Some(keys) = crate::preprocess::citation_keys(&after[..end]) {
                    out.push_str(&emoji_run(doc, &rest[..next.0]));
                    if keys.len() == 1 {
                        let key = keys[0];
                        let n = doc.cite_number(key);
                        out.push_str(&format!(
                            "<a class=\"md2pdf-cite\" href=\"#md2pdf-ref-{}\">[{n}]</a>",
                            esc_attr(key)
                        ));
                    } else {
                        out.push_str("<span class=\"md2pdf-cite\">[");
                        for (i, key) in keys.iter().enumerate() {
                            if i > 0 {
                                out.push_str(", ");
                            }
                            let n = doc.cite_number(key);
                            out.push_str(&format!(
                                "<a href=\"#md2pdf-ref-{}\">{n}</a>",
                                esc_attr(key)
                            ));
                        }
                        out.push_str("]</span>");
                    }
                    rest = &after[end + CITATION_CLOSE_TOKEN.len()..];
                    continue;
                }
            }
        }

        let after = &rest[next.0 + 2..];
        if let Some(end) = after.find("==") {
            let inner = &after[..end];
            if !inner.is_empty() && !inner.starts_with('=') && !inner.ends_with('=') {
                out.push_str(&emoji_run(doc, &rest[..next.0]));
                out.push_str(&format!("<mark>{}</mark>", emoji_run(doc, inner)));
                rest = &after[end + 2..];
                continue;
            }
        }
        out.push_str(&emoji_run(doc, &rest[..next.0 + 1]));
        rest = &rest[next.0 + 1..];
    }
    out.push_str(&emoji_run(doc, rest));
    out
}

/// Escape text, swapping emoji for the bundled Twemoji art when the host
/// supplied it. Without the asset the literal character is kept — browsers
/// have an emoji font, so that degrades gracefully.
fn emoji_run(doc: &Doc, s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < chars.len() {
        let Some((end, cp)) = match_emoji(&chars, i) else {
            plain.push(chars[i]);
            i += 1;
            continue;
        };
        let glyph: String = chars[i..end].iter().collect();
        match doc.assets.data_uri(&format!("twemoji/{cp}.svg")) {
            Some(src) => {
                out.push_str(&esc_text(&plain));
                plain.clear();
                out.push_str(&format!(
                    "<img class=\"md2pdf-emoji\" src=\"{src}\" alt=\"{}\" draggable=\"false\">",
                    esc_attr(&glyph)
                ));
            }
            None => plain.push_str(&glyph),
        }
        i = end;
    }
    out.push_str(&esc_text(&plain));
    out
}

/// Inline Markdown emphasis is not parsed in frontmatter values; they only
/// need escaping and emoji substitution.
fn inline_text(doc: &Doc, s: &str) -> String {
    emoji_run(doc, s)
}

// ==========================================================================
// Frontmatter & bibliography
// ==========================================================================

/// A numbered reference list for `bibliography: inline`.
///
/// Typst renders this with a real CSL style; reproducing CSL in the engine is
/// out of scope, so HTML emits an IEEE-shaped list of the entries that were
/// actually cited, in citation order.
fn bibliography(bibtex: &str, doc: &Doc) -> String {
    if bibtex.trim().is_empty() || doc.cites.is_empty() {
        return String::new();
    }
    let entries = parse_bibtex(bibtex);
    let heading = match &doc.bibliography_title {
        Some(title) => format!("<h2>{}</h2>", esc_text(title)),
        None => String::new(),
    };
    let mut out = format!("<section class=\"md2pdf-notes md2pdf-bibliography\">{heading}<ol>");
    for key in &doc.cites {
        let body = match entries.get(key) {
            Some(fields) => format_reference(fields),
            None => esc_text(key),
        };
        out.push_str(&format!(
            "<li id=\"md2pdf-ref-{}\">{body}</li>",
            esc_attr(key)
        ));
    }
    out.push_str("</ol></section>");
    out
}

/// The entry keys of a BibTeX source.
pub(crate) fn bibtex_keys(bib: &str) -> HashSet<String> {
    parse_bibtex(bib).into_keys().collect()
}

/// Minimal BibTeX reader: entry key plus `field = {value}` / `field = "value"`.
fn parse_bibtex(src: &str) -> HashMap<String, HashMap<String, String>> {
    let mut out = HashMap::new();
    let mut rest = src;
    while let Some(at) = rest.find('@') {
        rest = &rest[at + 1..];
        let Some(brace) = rest.find('{') else { break };
        let body = &rest[brace + 1..];
        let Some(len) = balanced_len(body) else { break };
        let entry = &body[..len];
        rest = &body[len..];
        let (key, fields) = entry.split_once(',').unwrap_or((entry, ""));
        out.insert(key.trim().to_string(), parse_fields(fields));
    }
    out
}

/// Length of the text up to the `}` matching an already-consumed `{`.
fn balanced_len(s: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_fields(src: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut rest = src;
    while let Some(eq) = rest.find('=') {
        let name = rest[..eq]
            .trim_start_matches([',', '\n', '\r', ' ', '\t'])
            .trim()
            .to_string();
        let after = rest[eq + 1..].trim_start();
        let (value, consumed) = match after.chars().next() {
            Some('{') => match balanced_len(&after[1..]) {
                Some(len) => (after[1..1 + len].to_string(), 1 + len + 1),
                None => break,
            },
            Some('"') => match after[1..].find('"') {
                Some(len) => (after[1..1 + len].to_string(), 1 + len + 1),
                None => break,
            },
            _ => {
                let len = after.find(',').unwrap_or(after.len());
                (after[..len].trim().to_string(), len)
            }
        };
        if !name.is_empty() {
            out.insert(
                name.to_ascii_lowercase(),
                value
                    .replace(['{', '}', '\n'], " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        let offset = rest.len() - after.len() + consumed;
        rest = &rest[offset.min(rest.len())..];
    }
    out
}

fn format_reference(fields: &HashMap<String, String>) -> String {
    let get = |k: &str| fields.get(k).map(String::as_str).unwrap_or_default();
    let mut parts: Vec<String> = Vec::new();
    let authors = get("author");
    if !authors.is_empty() {
        let names: Vec<String> = authors.split(" and ").map(abbreviate_name).collect();
        parts.push(esc_text(&names.join(", ")));
    }
    let title = get("title");
    if !title.is_empty() {
        parts.push(format!("\u{201c}{}\u{201d}", esc_text(title)));
    }
    let venue = [get("journal"), get("booktitle"), get("publisher")]
        .into_iter()
        .find(|v| !v.is_empty())
        .unwrap_or_default();
    if !venue.is_empty() {
        parts.push(format!("<em>{}</em>", esc_text(venue)));
    }
    for (label, key) in [("vol. ", "volume"), ("no. ", "number"), ("pp. ", "pages")] {
        let v = get(key);
        if !v.is_empty() {
            parts.push(format!("{label}{}", esc_text(v)));
        }
    }
    let year = get("year");
    if !year.is_empty() {
        parts.push(esc_text(year));
    }
    let mut out = parts.join(", ");
    if !out.is_empty() {
        out.push('.');
    }
    let url = get("url");
    if !url.is_empty() {
        if let Some(href) = safe_url(url) {
            out.push_str(&format!(
                " <a href=\"{}\">{}</a>",
                esc_attr(href),
                esc_text(url)
            ));
        }
    }
    out
}

/// `Ada Lovelace` -> `A. Lovelace`; `Lovelace, Ada` -> `A. Lovelace`.
fn abbreviate_name(name: &str) -> String {
    let name = name.trim();
    let (first, last) = match name.split_once(',') {
        Some((last, first)) => (first.trim().to_string(), last.trim().to_string()),
        None => match name.rsplit_once(' ') {
            Some((first, last)) => (first.trim().to_string(), last.trim().to_string()),
            None => return name.to_string(),
        },
    };
    let initials: String = first
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .map(|c| format!("{c}. "))
        .collect();
    format!("{initials}{last}")
}

// ==========================================================================
// Escaping
// ==========================================================================

pub(crate) fn esc_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

pub(crate) fn esc_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests;
