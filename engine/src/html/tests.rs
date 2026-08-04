//! Feature coverage for the HTML renderer, plus a parity guard against the
//! Typst renderer so a syntax feature cannot be added to one and forgotten in
//! the other.

use super::*;
use crate::convert_str;

/// Drop the inlined stylesheet and script, so assertions and `matches()`
/// counts see only the document markup.
fn strip_chrome(out: &str) -> String {
    let start = out.rfind("</style>").map_or(0, |i| i + "</style>".len());
    let end = out[start..].rfind("<script>").map_or(out.len(), |i| start + i);
    out[start..end].to_string()
}

/// Render a fragment with no host-supplied assets.
fn html(md: &str) -> String {
    strip_chrome(&render(md, "", "", b""))
}

/// Render with one asset available under `key`.
fn html_with(md: &str, key: &str, bytes: &[u8]) -> String {
    strip_chrome(&render(md, "", &format!("{key}\t{}\n", bytes.len()), bytes))
}

/// The `<div class="md2pdf-body">` contents, so assertions ignore the shell.
fn body(md: &str) -> String {
    let out = html(md);
    let open = "<div class=\"md2pdf-body\">";
    let start = out.find(open).expect("body") + open.len();
    let end = out[start..]
        .find("</div><section")
        .or_else(|| out[start..].find("</div></main>"))
        .expect("body end");
    out[start..start + end].to_string()
}

/// Elements the renderer is allowed to emit. Anything else in the output came
/// from the source document, which means escaping failed.
const ALLOWED_TAGS: &[&str] = &[
    "!doctype", "html", "head", "meta", "title", "style", "script", "body", "div", "main",
    "header", "section", "nav", "aside", "article", "p", "span", "h1", "h2", "h3", "h4", "h5",
    "h6", "a", "em", "strong", "del", "mark", "sup", "sub", "u", "br", "hr", "code", "pre",
    "button", "ul", "ol", "li", "input", "label", "table", "thead", "tbody", "tr", "th", "td",
    "colgroup", "col", "blockquote", "details", "summary", "figure", "figcaption", "img", "svg",
    "g", "nobr",
];

/// Assert that every tag in `out` is one the renderer emits — a whitelist
/// beats grepping for individual payloads, which escaped text keeps matching.
fn assert_no_injected_tags(out: &str) {
    let mut rest = out;
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '!' || *c == '/')
            .collect();
        let name = name.trim_start_matches('/').to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        // MathML elements all start with `m`; math-core is trusted output.
        let mathml = name.starts_with('m') && name.len() <= 12;
        assert!(
            ALLOWED_TAGS.contains(&name.as_str()) || mathml,
            "unexpected <{name}> in output:\n{out}"
        );
    }
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";

// ---- structure ----------------------------------------------------------

#[test]
fn a_fragment_carries_its_own_styles_and_root() {
    let out = render("# Title\n\ntext", "", "", b"");
    assert!(out.starts_with("<style>"), "{out}");
    assert!(out.contains("<div class=\"md2pdf\" id=\"md2pdf-root\" lang=\"en\">"));
    assert!(out.trim_end().ends_with("</script>"));
    assert!(!out.contains("<!doctype"));
}

#[test]
fn standalone_wraps_the_fragment_in_a_document() {
    let out = render("# Hello\n\ntext", "standalone=1\n", "", b"");
    assert!(out.starts_with("<!doctype html>"), "{out}");
    assert!(out.contains("<meta name=\"viewport\""));
    assert!(out.contains("<title>Hello</title>"));
    assert!(out.trim_end().ends_with("</html>"));
}

#[test]
fn standalone_is_off_by_default_and_for_an_explicit_zero() {
    assert!(!render("# a", "standalone=0\n", "", b"").starts_with("<!doctype"));
    assert!(!render("# a", "", "", b"").starts_with("<!doctype"));
}

#[test]
fn a_leading_h1_becomes_the_title_and_leaves_the_body() {
    let out = html("# The Title\n\nbody text");
    assert!(out.contains("<header class=\"md2pdf-titleblock\"><h1>The Title</h1>"), "{out}");
    assert!(!body("# The Title\n\nbody text").contains("The Title"));
}

#[test]
fn frontmatter_title_wins_and_keeps_the_h1_out_of_the_body() {
    let out = html(
        "---\ntitle: From Frontmatter\nauthors: [Ada, Grace]\ndate: 2026-01-01\n---\n\n# Ignored\n\nbody",
    );
    assert!(out.contains("<h1>From Frontmatter</h1>"), "{out}");
    assert!(out.contains("<span>Ada</span><span>Grace</span>"), "{out}");
    assert!(out.contains("<span>2026-01-01</span>"), "{out}");
    assert!(!out.contains("Ignored"), "{out}");
}

#[test]
fn frontmatter_reads_block_lists_and_quoted_scalars() {
    let out = html("---\ntitle: \"Quoted Title\"\nauthors:\n  - Ada\n  - Grace\n---\n\ntext");
    assert!(out.contains("<h1>Quoted Title</h1>"), "{out}");
    assert!(out.contains("<span>Ada</span><span>Grace</span>"), "{out}");
}

#[test]
fn german_documents_get_german_labels() {
    let out = html("---\nlang: de-AT\n---\n\n# A\n\n## B\n\n## C\n\n:::info\nx\n:::\n\ntext[^n]\n\n[^n]: note");
    assert!(out.contains("lang=\"de-AT\""), "{out}");
    assert!(out.contains(">Info<"), "{out}");
    assert!(out.contains(">Inhalt<"), "{out}");
    assert!(out.contains("Fußnoten"), "{out}");
}

#[test]
fn a_document_with_nothing_in_it_still_renders() {
    let out = html("");
    assert!(out.contains("class=\"md2pdf-body\""), "{out}");
    assert!(!out.contains("md2pdf-titleblock"));
    assert!(!out.contains("md2pdf-toc-btn"));
}

#[test]
fn a_frontmatter_only_document_renders_just_the_title_block() {
    let out = html("---\ntitle: Only\n---\n");
    assert!(out.contains("<h1>Only</h1>"), "{out}");
}

// ---- text ---------------------------------------------------------------

#[test]
fn inline_marks_map_to_semantic_elements() {
    let out = body("*a* **b** ~~c~~ ==d== ^e^ ~f~ __g__ `h`");
    assert!(out.contains("<em>a</em>"), "{out}");
    assert!(out.contains("<strong>b</strong>"), "{out}");
    assert!(out.contains("<del>c</del>"), "{out}");
    assert!(out.contains("<mark>d</mark>"), "{out}");
    assert!(out.contains("<sup>e</sup>"), "{out}");
    assert!(out.contains("<sub>f</sub>"), "{out}");
    assert!(out.contains("<u>g</u>"), "{out}");
    assert!(out.contains("<code>h</code>"), "{out}");
}

#[test]
fn html_reflows_soft_breaks_but_keeps_hard_ones() {
    assert_eq!(body("one\ntwo"), "<p>one two</p>");
    assert!(body("one  \ntwo").contains("one<br>two"));
}

#[test]
fn headings_get_stable_unique_ids_and_an_anchor() {
    let out = body("## Same\n\n### Same\n\n#### Ünï cödé!");
    assert!(out.contains("<h2 id=\"same\">"), "{out}");
    assert!(out.contains("<h3 id=\"same-2\">"), "{out}");
    assert!(out.contains("id=\"ünï-cödé\""), "{out}");
    assert!(out.contains("<a class=\"md2pdf-anchor\" href=\"#same\""), "{out}");
}

#[test]
fn a_heading_of_only_punctuation_still_gets_an_id() {
    assert!(body("## ***").contains("id=\"section\""));
}

#[test]
fn thematic_breaks_and_page_break_tokens_are_distinguishable() {
    assert_eq!(body("***"), "<hr>");
    assert_eq!(body("[[pagebreak]]"), "<hr class=\"md2pdf-pagebreak\">");
    assert_eq!(
        body("a\n\n\n\n\nb"),
        "<p>a</p><div class=\"md2pdf-spacer\"></div><p>b</p>"
    );
}

// ---- lists --------------------------------------------------------------

#[test]
fn lists_nest_and_keep_their_ordinal_start() {
    let out = body("- a\n  - b\n\n1. x\n2. y");
    assert!(out.contains("<ul><li>a<ul><li>b</li></ul></li></ul>"), "{out}");
    assert!(out.contains("<ol><li>x</li><li>y</li></ol>"), "{out}");
    assert!(body("5. five").contains("<ol start=\"5\">"));
}

#[test]
fn task_lists_render_real_checkboxes() {
    let out = body("- [x] done\n- [ ] open");
    assert!(out.contains("<ul class=\"md2pdf-tasks\">"), "{out}");
    assert!(out.contains("<input type=\"checkbox\" disabled checked><div>done</div>"), "{out}");
    assert!(out.contains("<input type=\"checkbox\" disabled><div>open</div>"), "{out}");
}

#[test]
fn a_loose_list_item_keeps_its_paragraphs() {
    let out = body("- one\n\n  two\n");
    assert!(out.contains("<li>one<p>two</p></li>"), "{out}");
}

#[test]
fn deep_nesting_does_not_blow_up() {
    let md = (0..8)
        .map(|d| format!("{}- l{d}", "  ".repeat(d)))
        .collect::<Vec<_>>()
        .join("\n");
    let out = body(&md);
    assert_eq!(out.matches("<ul>").count(), 8, "{out}");
    assert_eq!(out.matches("</ul>").count(), 8);
}

// ---- blocks -------------------------------------------------------------

#[test]
fn blockquotes_and_admonitions_carry_their_kind() {
    assert!(body("> quoted").contains("<blockquote><p>quoted</p></blockquote>"));
    let out = body(":::warning Careful\nbody\n:::");
    assert!(out.contains("<aside class=\"md2pdf-adm md2pdf-adm-warning\">"), "{out}");
    assert!(out.contains("md2pdf-adm-label\">Careful</strong><p>body</p>"), "{out}");
}

#[test]
fn an_untitled_admonition_falls_back_to_its_default_label() {
    assert!(body(":::danger\nx\n:::").contains(">DANGER<"));
    assert!(body(":::important\nx\n:::").contains(">IMPORTANT<"));
}

#[test]
fn admonitions_nest() {
    let out = body("::::info Outer\n:::tip Inner\ndeep\n:::\n::::");
    assert!(out.contains("md2pdf-adm-info"), "{out}");
    assert!(out.contains("md2pdf-adm-tip"), "{out}");
    assert!(out.contains("deep"), "{out}");
}

#[test]
fn spoilers_become_native_details() {
    let out = body("+++++ Show me\nhidden\n+++++");
    assert!(
        out.contains("<details open><summary>Show me</summary><p>hidden</p></details>"),
        "{out}"
    );
}

#[test]
fn layout_directives_map_to_alignment_and_grid() {
    assert!(body(":::center\nmid\n:::").contains("<div class=\"md2pdf-center\"><p>mid</p></div>"));
    let out = body("::::row\nleft\n\nright\n::::");
    assert!(out.contains("style=\"--md-cols:2\""), "{out}");
    assert!(out.contains("<div><p>left</p></div><div><p>right</p></div>"), "{out}");
}

#[test]
fn a_six_column_row_reports_its_column_count() {
    let cells = (1..=6).map(|i| format!("c{i}")).collect::<Vec<_>>().join("\n\n");
    assert!(body(&format!("::::row\n{cells}\n::::")).contains("--md-cols:6"));
}

// ---- code ---------------------------------------------------------------

#[test]
fn code_blocks_get_a_language_tag_lines_and_a_copy_button() {
    let out = body("```rust\nlet x = 1;\nlet y = 2;\n```");
    assert!(out.contains("<div class=\"md2pdf-code\" data-lang=\"rust\">"), "{out}");
    assert!(
        out.contains("<button class=\"md2pdf-copy\" type=\"button\" data-done=\"Copied\">Copy</button>"),
        "{out}"
    );
    assert_eq!(out.matches("class=\"md2pdf-line\"").count(), 2, "{out}");
    assert!(out.contains("md2pdf-t-k"), "{out}");
}

#[test]
fn a_plain_fence_has_no_language_tag() {
    let out = body("```\nplain\n```");
    assert!(!out.contains("data-lang"), "{out}");
    assert!(out.contains("<span class=\"md2pdf-line\">plain</span>"), "{out}");
}

#[test]
fn code_content_is_escaped() {
    let out = body("```\n<script>alert(1)</script>\n```");
    assert!(!out.contains("<script>"), "{out}");
    assert!(out.contains("&lt;script&gt;"), "{out}");
}

#[test]
fn an_unclosed_fence_still_produces_one_code_block() {
    let out = body("text\n\n```js\nconst a = 1;");
    assert_eq!(out.matches("md2pdf-code").count(), 1, "{out}");
}

// ---- tables -------------------------------------------------------------

#[test]
fn tables_scroll_and_keep_their_alignment() {
    let out = body("| a | b |\n| :- | ---: |\n| 1 | 2 |");
    assert!(out.contains("<div class=\"md2pdf-table\"><table>"), "{out}");
    assert!(
        out.contains("<thead><tr><th>a</th><th align=\"right\">b</th></tr></thead>"),
        "{out}"
    );
    assert!(
        out.contains("<tbody><tr><td>1</td><td align=\"right\">2</td></tr></tbody>"),
        "{out}"
    );
}

#[test]
fn plus_width_markers_become_column_percentages() {
    let out = body("| a | b |\n| --- | ---+ |\n| 1 | 2 |");
    assert!(out.contains("<colgroup>"), "{out}");
    assert!(out.contains("width:33.3333%"), "{out}");
    assert!(out.contains("width:66.6667%"), "{out}");
}

#[test]
fn an_even_table_needs_no_colgroup() {
    assert!(!body("| a | b |\n| - | - |\n| 1 | 2 |").contains("<colgroup>"));
}

#[test]
fn a_very_wide_table_keeps_every_column() {
    let cols = 30;
    let head = (0..cols).map(|i| format!("c{i}")).collect::<Vec<_>>().join(" | ");
    let sep = vec!["-"; cols].join(" | ");
    let out = body(&format!("| {head} |\n| {sep} |\n| {head} |"));
    assert_eq!(out.matches("<th>").count(), cols);
}

// ---- links, images, media ----------------------------------------------

#[test]
fn external_links_are_marked_and_relative_ones_are_not() {
    assert!(body("[x](https://e.com)")
        .contains("<a href=\"https://e.com\" rel=\"noopener noreferrer\">x</a>"));
    assert!(body("[x](./a.md)").contains("<a href=\"./a.md\">x</a>"));
    assert!(body("[x](#frag)").contains("<a href=\"#frag\">x</a>"));
}

#[test]
fn an_empty_link_label_falls_back_to_the_url() {
    assert!(body("[](https://e.com/a)").contains(">https://e.com/a</a>"));
}

#[test]
fn scripting_url_schemes_are_refused() {
    for url in ["javascript:alert(1)", "JavaScript:alert(1)", "vbscript:x"] {
        let out = body(&format!("[click]({url})"));
        assert!(!out.contains("<a href"), "{url} -> {out}");
        assert!(out.contains("md2pdf-missing"), "{url} -> {out}");
    }
}

#[test]
fn images_embed_as_data_uris_with_their_caption() {
    let out = html_with("![A cat](images/cat.png)", "images/cat.png", PNG);
    assert!(out.contains("<figure><img src=\"data:image/png;base64,"), "{out}");
    assert!(out.contains("alt=\"A cat\""), "{out}");
    assert!(out.contains("<figcaption>A cat</figcaption>"), "{out}");
    assert!(out.contains("style=\"width:100%\""), "{out}");
}

#[test]
fn image_dimension_syntax_becomes_width_and_height() {
    let out = html_with("![](images/a.png \"=200x120\")", "images/a.png", PNG);
    assert!(out.contains("width=\"200\" height=\"120\""), "{out}");
    assert!(!out.contains("<figcaption>"), "{out}");
    // The angle-bracket form puts the spec in the URL field instead.
    let out = html_with("![alt](<images/a.png =300x>)", "images/a.png", PNG);
    assert!(out.contains("width=\"300\""), "{out}");
    assert!(!out.contains("height="), "{out}");
    assert!(out.contains("<figcaption>alt</figcaption>"), "{out}");
}

#[test]
fn a_missing_image_degrades_to_a_visible_placeholder() {
    let out = body("![alt text](images/gone.png)");
    assert!(out.contains("<span class=\"md2pdf-missing\">alt text</span>"), "{out}");
}

#[test]
fn remote_images_resolve_through_the_url_hash() {
    let key = format!("remote/{}", crate::hash_url("https://e.com/a.png"));
    let out = html_with("![](https://e.com/a.png)", &key, PNG);
    assert!(out.contains("data:image/png;base64,"), "{out}");
}

#[test]
fn a_figure_is_never_nested_inside_a_paragraph() {
    let out = html_with("![a](images/a.png)", "images/a.png", PNG);
    assert!(!out.contains("<p><figure"), "{out}");
}

#[test]
fn mermaid_inlines_the_hosts_svg_and_falls_back_to_code() {
    let key = mermaid_key("graph LR\nA-->B");
    let out = html_with("```mermaid\ngraph LR\nA-->B\n```", &key, b"<svg><g/></svg>");
    assert!(out.contains("<div class=\"md2pdf-mermaid\"><svg><g/></svg></div>"), "{out}");
    assert!(body("```mermaid\ngraph LR\n```").contains("data-lang=\"mermaid\""));
}

#[test]
fn a_scriptable_mermaid_svg_is_refused() {
    let key = mermaid_key("graph LR");
    let out = html_with("```mermaid\ngraph LR\n```", &key, b"<svg><script>x</script></svg>");
    assert!(!out.contains("<script>x"), "{out}");
    assert!(out.contains("data-lang=\"mermaid\""), "{out}");
}

#[test]
fn emoji_use_twemoji_art_when_available_and_the_character_otherwise() {
    let out = html_with("hi \u{1f600}", "twemoji/1f600.svg", b"<svg/>");
    assert!(out.contains("<img class=\"md2pdf-emoji\""), "{out}");
    assert!(out.contains("alt=\"\u{1f600}\""), "{out}");
    assert!(body("hi \u{1f600}").contains("hi \u{1f600}"));
}

#[test]
fn shortcodes_and_zwj_sequences_resolve_to_one_glyph() {
    assert!(body(":smile:").contains('\u{1f604}'));
    let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";
    let key = "twemoji/1f468-200d-1f469-200d-1f467.svg";
    assert_eq!(html_with(family, key, b"<svg/>").matches("md2pdf-emoji").count(), 1);
}

// ---- math ---------------------------------------------------------------

#[test]
fn math_becomes_mathml_in_both_modes() {
    assert!(body("$a+b$").contains("<math"));
    let out = body("$$\\frac{a}{b}$$");
    assert!(out.contains("md2pdf-math-block"), "{out}");
    assert!(out.contains("<mfrac>"), "{out}");
}

#[test]
fn math_with_markup_characters_stays_escaped() {
    let out = body("$a < b$");
    assert!(out.contains("<math"), "{out}");
    assert!(!out.contains("<b>"), "{out}");
}

// ---- footnotes, citations ----------------------------------------------

#[test]
fn footnotes_collect_into_a_numbered_section_with_backlinks() {
    let out = html("text[^a] more[^b]\n\n[^a]: first\n[^b]: second");
    assert!(out.contains("id=\"md2pdf-fnref-1\"><a href=\"#md2pdf-fn-1\">[1]</a>"), "{out}");
    assert!(out.contains("<li id=\"md2pdf-fn-2\"><p>second<a class=\"md2pdf-backref\""), "{out}");
    assert!(out.contains("href=\"#md2pdf-fnref-2\""), "{out}");
}

#[test]
fn a_footnote_referenced_twice_keeps_one_number() {
    let out = html("a[^n] b[^n]\n\n[^n]: once");
    assert_eq!(out.matches("<li id=\"md2pdf-fn-").count(), 1, "{out}");
    assert_eq!(out.matches(">[1]</a>").count(), 2, "{out}");
}

#[test]
fn a_self_referencing_footnote_terminates() {
    let out = html("a[^n]\n\n[^n]: see[^n]");
    assert!(out.contains("md2pdf-fn-1"), "{out}");
}

#[test]
fn an_undefined_footnote_reference_is_dropped() {
    assert!(!body("text[^missing]").contains("md2pdf-fnref"));
}

#[test]
fn inline_citations_number_by_first_use_and_build_a_reference_list() {
    let md = "---\nbibliography: inline\n---\n\nSee [@b] and [@a], then [@b].\n\n\
        @article{a, author = {Ada Lovelace}, title = {Notes}, journal = {Memoirs}, year = {1843}}\n\
        @book{b, author = {Grace Hopper and Jean Bartik}, title = {Compilers}, publisher = {ACM}, year = {1952}}\n";
    let out = html(md);
    assert!(out.contains("href=\"#md2pdf-ref-b\">[1]</a>"), "{out}");
    assert!(out.contains("href=\"#md2pdf-ref-a\">[2]</a>"), "{out}");
    assert!(out.contains("<li id=\"md2pdf-ref-b\">G. Hopper, J. Bartik"), "{out}");
    assert!(out.contains("\u{201c}Compilers\u{201d}"), "{out}");
    assert!(out.contains("<em>ACM</em>"), "{out}");
    assert!(out.contains("<li id=\"md2pdf-ref-a\">A. Lovelace"), "{out}");
    assert_eq!(out.matches("<li id=\"md2pdf-ref-").count(), 2, "{out}");
}

#[test]
fn citations_stay_literal_without_the_frontmatter_switch() {
    let out = body("See [@key].");
    assert!(out.contains("[@key]"), "{out}");
    assert!(!out.contains("md2pdf-cite"), "{out}");
}

// ---- table of contents --------------------------------------------------

#[test]
fn the_drawer_appears_once_there_are_two_headings_and_is_closed_by_default() {
    assert!(!html("## Only one").contains("md2pdf-toc-btn"));
    let out = html("## One\n\n### Two");
    assert!(out.contains("<input type=\"checkbox\" class=\"md2pdf-toc-state\""), "{out}");
    assert!(!out.contains(" checked"), "{out}");
    assert!(out.contains("<li data-level=\"0\"><a href=\"#one\">One</a></li>"), "{out}");
    assert!(out.contains("<li data-level=\"1\"><a href=\"#two\">Two</a></li>"), "{out}");
}

#[test]
fn a_toc_marker_becomes_an_inline_outline() {
    let out = html("# T\n\n[toc]\n\n## A\n\n## B");
    assert!(out.contains("<nav class=\"md2pdf-toc-inline\">"), "{out}");
    assert!(!out.contains(TOC_SLOT), "{out}");
    assert!(out.contains("href=\"#a\""), "{out}");
}

// ---- escaping & injection ----------------------------------------------

#[test]
fn markup_in_prose_alt_titles_and_headings_is_escaped() {
    for md in [
        "<script>alert(1)</script>",
        "## <img src=x onerror=alert(1)>",
        "![<script>bad</script>](images/x.png)",
        "> <iframe src=x>",
        "| <script>a</script> |\n| - |\n| <b onclick=x> |",
        "---\ntitle: <script>t</script>\nauthors: [<img src=x>]\n---\n",
        "+++++ <script>s</script>\nbody\n+++++",
        ":::info <object data=x>\nbody\n:::",
    ] {
        assert_no_injected_tags(&html(md));
    }
}

#[test]
fn raw_html_blocks_are_shown_not_executed() {
    let out = body("<div onclick=\"x\">raw</div>");
    assert!(out.contains("&lt;div onclick=\"x\"&gt;raw&lt;/div&gt;"), "{out}");
}

#[test]
fn the_narrow_inline_html_passthrough_still_works() {
    let out = body("a <u>b</u> c<br>d");
    assert!(out.contains("a <u>b</u> c<br>d"), "{out}");
}

#[test]
fn ampersands_are_escaped_exactly_once() {
    assert!(body("a & b").contains("a &amp; b"));
}

// ---- assets protocol ----------------------------------------------------

#[test]
fn discovery_lists_only_local_images() {
    let md = "![a](images/a.png) ![b](https://e.com/b.png) ![c](sub/c.jpg)";
    assert_eq!(local_images(md), vec!["images/a.png", "sub/c.jpg"]);
}

#[test]
fn discovery_finds_mermaid_inside_nested_blocks_but_not_inside_wider_fences() {
    let md = "```mermaid\ntop\n```\n\n:::info\n```mermaid\nnested\n```\n:::\n\n\
        ````md\n```mermaid\nquoted\n```\n````";
    assert_eq!(crate::collect_mermaid_sources(md), vec!["top", "nested"]);
}

#[test]
fn duplicate_diagrams_share_one_asset() {
    let md = "```mermaid\nsame\n```\n\n```mermaid\nsame\n```";
    assert_eq!(crate::collect_mermaid_sources(md).len(), 1);
    let out = html_with(md, &mermaid_key("same"), b"<svg/>");
    assert_eq!(out.matches("md2pdf-mermaid").count(), 2, "{out}");
}

#[test]
fn a_garbled_manifest_does_not_panic() {
    for manifest in ["nonsense", "k\tnotanumber", "k\t99999", "\t\t\t"] {
        let _ = render("![a](images/a.png)", "", manifest, b"ab");
    }
}

// ---- parity with the Typst renderer ------------------------------------

/// Every construct must produce output in *both* renderers. A feature added
/// to one and forgotten in the other trips this.
#[test]
fn every_construct_renders_in_both_outputs() {
    let cases: &[(&str, &str, &str)] = &[
        ("heading", "## H", "<h2"),
        ("emphasis", "*a*", "<em>"),
        ("strong", "**a**", "<strong>"),
        ("strike", "~~a~~", "<del>"),
        ("mark", "==a==", "<mark>"),
        ("super", "^a^", "<sup>"),
        ("sub", "~a~", "<sub>"),
        ("underline", "__a__", "<u>"),
        ("inline code", "`a`", "<code>"),
        ("link", "[a](b)", "<a href"),
        ("image", "![a](b.png)", "md2pdf-missing"),
        ("bullet list", "- a", "<ul>"),
        ("ordered list", "1. a", "<ol>"),
        ("task list", "- [x] a", "md2pdf-task"),
        ("table", "| a |\n| - |\n| 1 |", "<table>"),
        ("blockquote", "> a", "<blockquote>"),
        ("code block", "```\na\n```", "md2pdf-code"),
        ("thematic break", "***", "<hr>"),
        ("footnote", "a[^n]\n\n[^n]: d", "md2pdf-fnref"),
        ("math inline", "$a$", "<math"),
        ("math block", "$$a$$", "md2pdf-math-block"),
        ("admonition", ":::tip\na\n:::", "md2pdf-adm-tip"),
        ("spoiler", "+++++ s\na\n+++++", "<details"),
        ("align", ":::center\na\n:::", "md2pdf-center"),
        ("row", "::::row\na\n\nb\n::::", "md2pdf-row"),
        ("mermaid", "```mermaid\na\n```", "mermaid"),
        ("toc", "[toc]\n\n## a\n\n## b", "md2pdf-toc-inline"),
        ("pagebreak", "[[pagebreak]]", "md2pdf-pagebreak"),
        ("emoji", "\u{1f600}", "\u{1f600}"),
        ("autolink", "<https://e.com>", "<a href"),
    ];
    for (name, md, marker) in cases {
        assert!(!convert_str(md, false).trim().is_empty(), "{name}: empty Typst output");
        let out = html(md);
        assert!(out.contains(marker), "{name}: HTML missing {marker}\n{out}");
    }
}

/// The feature demo is the reference document; both renderers must swallow it
/// whole, and the HTML shell must survive intact.
#[test]
fn the_feature_demo_renders_in_both_outputs() {
    let md = include_str!("../../../tests/extended.md");
    assert!(!convert_str(md, false).is_empty());
    let out = render(md, "standalone=1\n", "", b"");
    assert!(out.starts_with("<!doctype html>"));
    assert!(out.trim_end().ends_with("</html>"));
    assert!(out.contains("md2pdf-toc-btn"), "the demo has headings");
    assert_eq!(out.matches("<div class=\"md2pdf-body\">").count(), 1);
    assert!(!out.contains(TOC_SLOT));
}

#[test]
fn the_edge_case_fixture_renders_without_panicking() {
    let md = include_str!("../../../tests/html-edge.md");
    let out = render(md, "standalone=1\n", "", b"");
    assert!(out.trim_end().ends_with("</html>"));
    assert_no_injected_tags(&strip_chrome(&out));
    // The fixture ends in a BibTeX block, so citations must have resolved.
    assert!(out.contains("md2pdf-ref-knuth"), "{}", strip_chrome(&out));
}
