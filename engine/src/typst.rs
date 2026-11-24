//! Markdown -> Typst markup, the PDF renderer.

use crate::frontmatter::Frontmatter;
use crate::html::math::Math;
use crate::preprocess::*;
use crate::rawhtml::{self, Raw};
use crate::*;
use comrak::nodes::{AstNode, ListType, NodeValue, TableAlignment};
use comrak::Arena;
use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};

/// Document-wide state, shared by the top-level parse and every admonition,
/// spoiler and row body re-parsed inside it.
pub(crate) struct Doc {
    pub(crate) citations: bool,
    /// Directory prefix of the `<dir>/<hash>` aliases for remote images.
    pub(crate) remote_dir: String,
    /// Heading ids already emitted, for stable duplicate suffixes.
    slugs: RefCell<HashMap<String, usize>>,
    /// Built on the first formula: most documents have none.
    math: OnceCell<Math>,
    /// Every footnote definition in the document, for references made in a
    /// different block than the one defining them.
    notes: HashMap<String, String>,
    /// `image-width`: the width of an image without a size of its own.
    image_width: String,
    /// `breaks: false`: a source line break is a space, not a line break.
    soft_breaks: bool,
    /// Footnotes currently being rendered. A note that references itself would
    /// otherwise recurse until the plugin's stack is exhausted.
    rendering_notes: RefCell<HashSet<String>>,
}

impl Doc {
    pub(crate) fn new(citations: bool, remote_dir: &str) -> Self {
        Self {
            citations,
            remote_dir: remote_dir.to_string(),
            slugs: RefCell::new(HashMap::new()),
            math: OnceCell::new(),
            notes: HashMap::new(),
            rendering_notes: RefCell::new(HashSet::new()),
            image_width: "100%".to_string(),
            soft_breaks: false,
        }
    }
}

/// Full Markdown -> Typst markup pipeline.
pub(crate) fn convert(src: &str, strip_h1: bool, doc: &mut Doc) -> String {
    doc.notes = footnote_sources(src);
    let fm = Frontmatter::parse(src);
    if let Some(width) = image_width(&fm) {
        doc.image_width = width;
    }
    doc.soft_breaks = fm.get("breaks").is_some_and(crate::frontmatter::is_off);
    render_source(src, strip_h1, None, doc)
}

#[cfg(test)]
pub(crate) fn convert_str(src: &str, strip_h1: bool) -> String {
    convert_str_with_citations(src, strip_h1, false)
}

#[cfg(test)]
pub(crate) fn convert_str_with_citations(src: &str, strip_h1: bool, citations: bool) -> String {
    convert(src, strip_h1, &mut Doc::new(citations, "remote"))
}

/// Recursive: admonition and spoiler bodies are re-run through it so nested
/// custom syntax works.
fn render_source(src: &str, strip_h1: bool, alignment: Option<Alignment>, doc: &Doc) -> String {
    let citation_source = if doc.citations {
        preprocess_citations(src)
    } else {
        src.to_string()
    };
    let pre = preprocess(&citation_source);
    let arena = Arena::new();
    let markdown = with_note_stubs(&pre.markdown, &doc.notes);
    let root = parse(&arena, &markdown);
    let ctx = Ctx::new(pre, root, doc, alignment);
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
    list_styles: Vec<ListStyle>,
    /// Set by a `liststyle` placeholder, consumed by the list right after it.
    pending_style: std::cell::Cell<Option<ListStyle>>,
    doc: &'a Doc,
    alignment: Option<Alignment>,
}

impl<'a> Ctx<'a> {
    fn new(
        pre: Preprocessed,
        root: &'a AstNode<'a>,
        doc: &'a Doc,
        alignment: Option<Alignment>,
    ) -> Self {
        let mut ctx = Ctx {
            footnotes: HashMap::new(),
            admonitions: pre.admonitions,
            spoilers: pre.spoilers,
            table_widths: pre.table_widths,
            pending_widths: std::cell::Cell::new(None),
            list_styles: pre.list_styles,
            pending_style: std::cell::Cell::new(None),
            doc,
            alignment,
        };
        ctx.collect_footnotes(root);
        ctx
    }

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
        let data = node.data.borrow();
        let value = &data.value;
        // A pending width id belongs to the table that directly follows its
        // placeholder. If anything else intervenes the markers never became a
        // table, so drop them rather than let them land on a later one.
        if !matches!(value, NodeValue::Table(_) | NodeValue::HtmlBlock(_)) {
            self.pending_widths.set(None);
        }
        if !matches!(value, NodeValue::List(_) | NodeValue::HtmlBlock(_)) {
            self.pending_style.set(None);
        }
        let out = match value {
            NodeValue::Document => self.render_block_children(node),
            NodeValue::Heading(h) => {
                let level = h.level.clamp(1, 6) as usize;
                let (text, custom) = heading_parts(&plain_text(node));
                let id =
                    unique_heading_id(&mut self.doc.slugs.borrow_mut(), &text, custom.as_deref());
                format!(
                    "{} {} <{}>",
                    "=".repeat(level),
                    self.render_heading_inlines(node),
                    id
                )
            }
            NodeValue::Paragraph => self.render_paragraph(node),
            NodeValue::ThematicBreak(_) => "#line(length: 100%, stroke: 0.6pt)".to_string(),
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => {
                let body = self.render_block_children(node);
                if body.trim().is_empty() {
                    "#quote[]".to_string()
                } else {
                    format!("#quote[\n{}\n]", indent_lines(&body, 1))
                }
            }
            NodeValue::List(_) => {
                let list = self.render_list(node, indent);
                return match self.pending_style.take() {
                    // Lettered and roman lists count in their own style.
                    Some(style) => {
                        let base = "  ".repeat(indent);
                        format!(
                            "{base}#[\n{base}#set enum(numbering: \"{}.\")\n{list}\n{base}]",
                            style.kind
                        )
                    }
                    None => list,
                };
            }
            NodeValue::Item(_) | NodeValue::TaskItem(_) => self.render_block_children(node),
            NodeValue::CodeBlock(cb) => {
                return self.render_code_block(&cb.info, &cb.literal, indent)
            }
            NodeValue::HtmlBlock(hb) => {
                if let Some(id) = parse_placeholder(&hb.literal, "admonition") {
                    self.render_admonition(id)
                } else if let Some(id) = parse_placeholder(&hb.literal, "spoiler") {
                    self.render_spoiler(id)
                } else if let Some(id) = parse_placeholder(&hb.literal, "tablewidths") {
                    self.pending_widths.set(Some(id));
                    String::new()
                } else if let Some(id) = parse_placeholder(&hb.literal, "liststyle") {
                    self.pending_style.set(self.list_styles.get(id).copied());
                    String::new()
                } else {
                    esc_text(&rawhtml::without_comments(&hb.literal))
                }
            }
            NodeValue::Table(_) => return self.render_table(node, indent),
            NodeValue::FootnoteDefinition(_) => String::new(),
            NodeValue::Math(m) => self.render_math(m.display_math, &m.literal),
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
                let inner = render_source(&a.source, false, Some(alignment), self.doc);
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
            "row" => render_row(&a.source, self.alignment, self.doc),
            // Styled callout box.
            _ => {
                let inner = render_source(&a.source, false, self.alignment, self.doc);
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
        let inner = render_source(&s.source, false, self.alignment, self.doc);
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
        let open = format!("{fence}{}", typst_lang(&code_language(info)));
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
        let ordered = nl.list_type == ListType::Ordered;
        node.children()
            .enumerate()
            .map(|(i, item)| {
                // An explicit number on the first item sets where Typst starts
                // counting; the `+` items after it continue from there.
                let marker = match (ordered, i) {
                    (true, 0) if nl.start > 1 => format!("{}.", nl.start),
                    (true, _) => "+".to_string(),
                    (false, _) => "-".to_string(),
                };
                self.render_list_item(item, &marker, indent)
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The inline content of an item's first paragraph. Its wrapped lines are
    /// indented past the marker, or Typst would end the item at each one.
    fn render_item_inlines(&self, paragraph: &'a AstNode<'a>, base: &str) -> String {
        self.render_inlines(paragraph)
            .replace('\n', &format!("\n{base}  "))
    }

    fn render_list_item(&self, item: &'a AstNode<'a>, marker: &str, indent: usize) -> String {
        let base = "  ".repeat(indent);
        let mut lines: Vec<String> = Vec::new();
        let mut first_done = false;
        for child in item.children() {
            let is_para = matches!(child.data.borrow().value, NodeValue::Paragraph);
            if is_para && !first_done {
                lines.push(format!(
                    "{base}{marker} {}",
                    self.render_item_inlines(child, &base)
                ));
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
            let data = child.data.borrow();
            match &data.value {
                NodeValue::Paragraph if !first_done => {
                    body = self.render_item_inlines(child, &base);
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
                    // An empty `*…*` is a Typst warning, not an empty cell.
                    if header && !inner.trim().is_empty() {
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
        self.render_nodes(&node.children().collect::<Vec<_>>())
    }

    /// Sibling inline nodes. A formatting tag from `rawhtml` and its closing
    /// tag wrap the nodes between them; an unpaired one stays visible text.
    fn render_nodes(&self, kids: &[&'a AstNode<'a>]) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < kids.len() {
            if let Some((tag, close)) = open_tag(kids, i) {
                out.push_str(&typst_tag(tag, &self.render_nodes(&kids[i + 1..close])));
                i = close + 1;
                continue;
            }
            out.push_str(&self.render_inline(kids[i]));
            i += 1;
        }
        out
    }

    /// A heading's content, minus a trailing `{#id}` — which may follow any
    /// inline markup, not only plain text.
    fn render_heading_inlines(&self, node: &'a AstNode<'a>) -> String {
        let kids: Vec<&AstNode> = node.children().collect();
        if let Some((last, visible)) = heading_text_without_id(node, &kids) {
            return self.render_nodes(&kids[..last]) + &render_text(&visible, self.doc.citations);
        }
        self.render_nodes(&kids)
    }

    fn render_inline(&self, node: &'a AstNode<'a>) -> String {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::Text(t) => render_text(t, self.doc.citations),
            // Soft breaks (source line wraps) become hard breaks, matching the
            // web app's pipeline — it preserves the author's line wrapping.
            NodeValue::SoftBreak if self.doc.soft_breaks => " ".to_string(),
            NodeValue::SoftBreak => "\\\n".to_string(),
            NodeValue::LineBreak => "\\\n".to_string(),
            NodeValue::Escaped => self.render_inlines(node),
            NodeValue::Emph(_) => format!("#emph[{}]", self.render_inlines(node)),
            NodeValue::Strong(_) => format!("#strong[{}]", self.render_inlines(node)),
            NodeValue::Strikethrough(_) => format!("#strike[{}]", self.render_inlines(node)),
            NodeValue::Superscript => format!("#super[{}]", self.render_inlines(node)),
            NodeValue::Subscript => format!("#sub[{}]", self.render_inlines(node)),
            NodeValue::Underline | NodeValue::Insert => {
                format!("#underline[{}]", self.render_inlines(node))
            }
            NodeValue::Code(c) => render_inline_code(&c.literal),
            NodeValue::Math(m) => self.render_math(m.display_math, &m.literal),
            NodeValue::HtmlInline(h) => match rawhtml::classify(h) {
                Raw::Comment => String::new(),
                Raw::Open("br") => "#linebreak()".to_string(),
                _ => esc_text(h),
            },
            NodeValue::ShortCode(s) => emit_text(&s.emoji),
            NodeValue::Link(l) => render_link(&l.url, &self.render_inlines(node)),
            NodeValue::Image(l) => render_image(
                &l.url,
                &l.title,
                &plain_text(node),
                self.visual_alignment(),
                self.doc,
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
        if !self
            .doc
            .rendering_notes
            .borrow_mut()
            .insert(name.to_string())
        {
            // Already inside this note: drop the back-reference rather than
            // recurse into it forever.
            return String::new();
        }
        // A note defined in another block is rendered from its source.
        let content = match self.doc.notes.get(name).filter(|_| is_note_stub(def)) {
            Some(source) => render_source(source, false, self.alignment, self.doc),
            None => def
                .children()
                .map(|c| self.render_block(c, 0))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        };
        self.doc.rendering_notes.borrow_mut().remove(name);
        format!("#footnote[{}]", content.trim())
    }

    /// A formula the PDF cannot typeset would abort the whole compile, so it is
    /// shown as its source instead, as the HTML renderer does.
    fn render_math(&self, display: bool, latex: &str) -> String {
        let alignment = self.visual_alignment();
        let latex = &mitex_symbols(latex);
        if self.doc.math.get_or_init(Math::new).pdf_accepts(latex) {
            return render_math(display, latex, alignment);
        }
        let error = format!("#md-math-error(\"{}\")", esc_string(latex.trim()));
        if display {
            format!("#align({})[{error}]", alignment.typst())
        } else {
            error
        }
    }

    fn visual_alignment(&self) -> Alignment {
        self.alignment.unwrap_or(Alignment::Center)
    }
}

/// Render a `:::row` block: each top-level child block becomes a grid column.
fn render_row(source: &str, alignment: Option<Alignment>, doc: &Doc) -> String {
    if source.trim().is_empty() {
        return String::new();
    }
    let pre = preprocess(source);
    let arena = Arena::new();
    let markdown = with_note_stubs(&pre.markdown, &doc.notes);
    let root = parse(&arena, &markdown);
    let ctx = Ctx::new(pre, root, doc, alignment);
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

/// Render a text run, turning `==mark==` spans into `#highlight[...]`.
/// Matches the former `remark-mark` plugin: flat text only, no nested markup.
fn render_text(s: &str, citations: bool) -> String {
    split_bylines(s)
        .into_iter()
        .map(|(byline, part)| {
            if byline {
                format!(
                    "#text(size: 0.85em, fill: luma(120), style: \"italic\")[{}]",
                    emit_text(part)
                )
            } else {
                render_marks(part, citations)
            }
        })
        .collect()
}

fn render_marks(s: &str, citations: bool) -> String {
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

/// mathtools' colon-equals commands, which mitex maps to a colon and a minus,
/// as the characters they stand for.
fn mitex_symbols(latex: &str) -> String {
    let mut out = String::with_capacity(latex.len());
    let mut rest = latex;
    while let Some(at) = rest.find('\\') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let len = after
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(after.len());
        let symbol = match &after[..len] {
            "coloneq" | "coloneqq" => Some("≔"),
            "eqqcolon" | "eqcolon" => Some("≕"),
            _ => None,
        };
        match symbol {
            Some(symbol) => {
                out.push_str(symbol);
                rest = &after[len..];
            }
            None => {
                let keep = 1 + len.max(after.chars().next().map_or(0, char::len_utf8));
                out.push_str(&rest[at..at + keep]);
                rest = &rest[at + keep..];
            }
        }
    }
    out.push_str(rest);
    out
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
        // Typst refuses an empty URL, and there is nothing to link to anyway.
        Some("") => label,
        Some(href) if href.starts_with('#') && href.len() > 1 => {
            format!("#md-jump(\"{}\")[{}]", esc_string(&href[1..]), label)
        }
        Some(href) => format!("#link(\"{}\")[{}]", esc_string(href), label),
        None => label,
    }
}

/// HackMD `=WxH` dimension syntax + remote-URL aliasing. An image the host
/// cannot load — an empty or `data:` target, a foreign scheme — gets the same
/// blank stand-in as a remote image whose fetch failed, rather than an
/// `image()` call that would abort the compile.
fn render_image(url: &str, title: &str, alt: &str, alignment: Alignment, doc: &Doc) -> String {
    let (url, dims) = html::split_dims(url, title);
    let path = if is_remote(&url) {
        Some(format!("{}/{}", doc.remote_dir, hash_url(&url)))
    } else {
        local_image_path(&url)
    };

    let mut args = Vec::new();
    match &dims {
        Some((w, h)) => {
            if let Some(w) = w {
                args.push(format!("width: {w}pt"));
            }
            if let Some(h) = h {
                args.push(format!("height: {h}pt"));
            }
        }
        None => args.push(format!("width: {}", doc.image_width)),
    }
    let image_call = match path {
        Some(path) => format!("#md-image(\"{}\", {})", esc_string(&path), args.join(", ")),
        None => format!("#md-missing-image({})", args.join(", ")),
    };
    let figure_width = dims
        .as_ref()
        .and_then(|(width, _)| width.as_ref())
        .map(|width| format!("{width}pt"))
        .unwrap_or_else(|| doc.image_width.clone());

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

/// A formatting tag's Typst equivalent around already-rendered content.
fn typst_tag(tag: &str, body: &str) -> String {
    let call = match tag {
        "b" | "strong" => "#strong",
        "i" | "em" => "#emph",
        "u" | "ins" => "#underline",
        "s" | "del" | "strike" => "#strike",
        "sub" => "#sub",
        "sup" => "#super",
        "kbd" => "#md-kbd",
        "mark" => "#highlight",
        _ => "#text(size: 0.85em)",
    };
    format!("{call}[{body}]")
}

/// A fence language as a Typst raw tag, which ends at the first character
/// that is not alphanumeric, `-` or `_` — the rest would become code. The
/// usual spellings with such characters get their syntax's own name.
fn typst_lang(lang: &str) -> &str {
    match lang.to_ascii_lowercase().as_str() {
        "c++" => "cpp",
        "c#" => "cs",
        "f#" => "fs",
        _ if lang
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') =>
        {
            lang
        }
        _ => "",
    }
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
///
/// `/` is escaped everywhere, or `//` and `/*` would open a comment. The start
/// of a run needs more: right after a `#call(…)[…]` a `.`, `(` or `;` would
/// continue the call, and at the start of a line (every run after a soft
/// break) `= ` or `2. ` would become a heading or an enum item.
fn esc_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    for (i, c) in s.char_indices() {
        let leading = (i == 0 && matches!(c, '.' | '(' | ';' | '='))
            || (digits > 0 && i == digits && c == '.');
        if leading
            || matches!(
                c,
                '\\' | '#'
                    | '*'
                    | '_'
                    | '`'
                    | '['
                    | ']'
                    | '$'
                    | '<'
                    | '>'
                    | '@'
                    | '~'
                    | '+'
                    | '-'
                    | '/'
            )
        {
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

    #[test]
    fn typst_markup_characters_remain_literal_prose() {
        assert_eq!(
            convert_str("1~2\n1 ~ 2\n1\\~2", false),
            "1\\~2\\\n1 \\~ 2\\\n1\\~2"
        );
        assert_eq!(convert_str("\\+ 111\n\\- 111", false), "\\+ 111\\\n\\- 111");
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
    fn semicolon_grouped_citations_work_like_commas() {
        let out = convert_str_with_citations("See [@first; @second].", false, true);
        assert!(
            out.contains("#md-cite-group((label(\"first\"), label(\"second\")))"),
            "{out}"
        );
    }

    /// HedgeDoc's line-number flags and pandoc's attribute forms used to land
    /// in the code as its first line.
    #[test]
    fn fence_info_beyond_the_language_stays_out_of_the_code() {
        for (info, lang) in [
            ("js=", "js"),
            ("js=101", "js"),
            ("js=+", "js"),
            ("js!", "js"),
            ("!", ""),
            ("c++", "cpp"),
            ("c#", "cs"),
            ("{.python}", "python"),
            ("python title=\"x.py\"", "python"),
            ("a.b", ""),
        ] {
            let out = convert_str(&format!("```{info}\ncode\n```"), false);
            assert_eq!(out, format!("```{lang}\ncode\n```"), "{info}");
        }
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

    /// mitex fails inside the Typst compile, where nothing can catch it, so
    /// one bad formula used to abort the whole PDF.
    #[test]
    fn a_formula_mitex_cannot_typeset_shows_its_source() {
        for latex in [r"\frac{a", r"\href{a}{b}", r"\text{a<b}"] {
            let out = convert_str(&format!("a ${latex}$ b"), false);
            assert!(out.contains("#md-math-error("), "{latex}: {out}");
            assert!(!out.contains("#md-math("), "{latex}: {out}");
        }
        let display = convert_str("$$\\frac{a$$", false);
        assert_eq!(display, "#align(center)[#md-math-error(\"\\\\frac{a\")]");
        assert!(convert_str("$\\frac{a}{b}$", false).contains("#md-math(false"));
    }

    /// Typst reads `.name` right after a call as field access, and `//` as a
    /// comment; either aborted the PDF or swallowed the rest of the line.
    #[test]
    fn text_after_a_call_never_continues_it() {
        for (md, tail) in [
            ("**bold**.next", "\\.next"),
            ("_it_.com", "\\.com"),
            ("$x$.y", "\\.y"),
            (":smile:.z", "\\.z"),
            ("**b**(x)", "\\(x)"),
            ("**b**; c", "\\; c"),
        ] {
            let out = convert_str(md, false);
            assert!(out.ends_with(tail), "{md}: {out}");
        }
        assert_eq!(
            convert_str("foo // bar /* baz", false),
            "foo \\/\\/ bar \\/\\* baz"
        );
        assert!(convert_str("# Title // x", false).starts_with("= Title \\/\\/ x <title-x>"));
    }

    /// A wrapped line is still part of its paragraph, whatever it starts with.
    #[test]
    fn a_wrapped_line_never_becomes_block_markup() {
        let out = convert_str(
            "para\n= not heading\n/ Term: desc\n2. two\n1990. then",
            false,
        );
        assert_eq!(
            out,
            "para\\\n\\= not heading\\\n\\/ Term: desc\\\n2\\. two\\\n1990\\. then"
        );
    }

    #[test]
    fn wrapped_list_items_keep_their_indent_and_numbering() {
        assert_eq!(
            convert_str("1. first\n   wraps\n2. second", false),
            "+ first\\\n  wraps\n+ second"
        );
        assert_eq!(
            convert_str("- a\n  - nested\n    wraps", false),
            "- a\n  - nested\\\n    wraps"
        );
        assert_eq!(
            convert_str("- [ ] a\n  b", false),
            "#task-item(false)[a\\\n  b]"
        );
    }

    #[test]
    fn an_ordered_list_keeps_its_start_number() {
        assert_eq!(convert_str("57. a\n58. b", false), "57. a\n+ b");
        assert_eq!(convert_str("1. a\n2. b", false), "+ a\n+ b");
    }

    #[test]
    fn a_task_is_checked_only_by_its_own_marker() {
        let out = convert_str("- [ ] parent\n  - [x] child", false);
        assert!(out.contains("#task-item(false)[parent]"), "{out}");
        assert!(out.contains("#task-item(true)[child]"), "{out}");
    }

    #[test]
    fn colon_equals_reaches_mitex_as_the_character() {
        assert_eq!(
            convert_str(r"$x \coloneq y \coloneqq z \eqqcolon w \colon v$", false),
            r#"#md-math(false, "x ≔ y ≔ z ≕ w \\colon v")"#
        );
    }

    #[test]
    fn an_empty_header_cell_has_no_strong_markup() {
        let out = convert_str("| a |  |\n|---|---|\n| 1 | 2 |", false);
        assert!(out.contains("table.header([*a*], [])"), "{out}");
    }

    #[test]
    fn inserted_text_and_inline_footnotes() {
        assert_eq!(
            convert_str("++ins++ C++ and C++", false),
            "#underline[ins] C\\+\\+ and C\\+\\+"
        );
        let out = convert_str("Text^[An inline note] and 2^10^.", false);
        assert!(out.contains("#footnote[An inline note]"), "{out}");
        assert!(out.contains("#super[10]"), "{out}");
    }

    #[test]
    fn inline_code_with_a_backtick_uses_the_function_form() {
        assert_eq!(render_inline_code("a`b"), "#raw(\"a`b\")");
        assert_eq!(render_inline_code("ab"), "`ab`");
    }

    #[test]
    fn keeps_visuals_centered_without_a_directive() {
        assert_eq!(
            render_image(
                "image.png",
                "=80x",
                "",
                Alignment::Center,
                &Doc::new(false, "remote")
            ),
            "#align(center)[#md-image(\"image.png\", width: 80pt)]"
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
            out.contains("#align(right)[#md-image(\"right.png\", width: 80pt)]"),
            "{out}"
        );
        assert!(
            out.contains("#align(center)[#md-image(\"center.png\", width: 80pt)]"),
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
            row.contains("#align(right)[#md-image(\"row.png\", width: 80pt)]"),
            "{row}"
        );

        let spoiler = convert_str(
            ":::left\n+++++ Summary\n![](spoiler.png \"=80x\")\n+++++\n:::\n",
            false,
        );
        assert!(
            spoiler.contains("#spoiler(summary: \"Summary\")"),
            "{spoiler}"
        );
        assert!(
            spoiler.contains("#align(left)[#md-image(\"spoiler.png\", width: 80pt)]"),
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
