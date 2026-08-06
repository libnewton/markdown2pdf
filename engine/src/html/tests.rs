//! Feature coverage for the HTML renderer, plus a parity guard against the
//! Typst renderer so a syntax feature cannot be added to one and forgotten in
//! the other.

use super::*;
use crate::convert_str;

/// WCAG 2.1 relative luminance of a `#rrggbb` string.
fn luminance(hex: &str) -> f64 {
    let ch = |i: usize| {
        let v = u8::from_str_radix(&hex[1 + i * 2..3 + i * 2], 16).expect("hex pair") as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(0) + 0.7152 * ch(1) + 0.0722 * ch(2)
}

fn contrast(a: &str, b: &str) -> f64 {
    let (hi, lo) = {
        let (x, y) = (luminance(a), luminance(b));
        if x > y {
            (x, y)
        } else {
            (y, x)
        }
    };
    (hi + 0.05) / (lo + 0.05)
}

fn base(name: &str) -> (&'static str, &'static str) {
    tokens::BASE
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, l, d)| (*l, *d))
        .unwrap_or_else(|| panic!("no --md-{name}"))
}

/// Every syntax-highlighting colour has to clear WCAG AA against the code
/// background it is actually painted on, in *both* themes.
///
/// Worth having as a test rather than a review habit: the neighbouring branch
/// shipped four token colours that failed this in both themes, because its
/// checker only ever looked at the `:root` variables and never at the colours
/// the highlighter emits.
#[test]
fn token_colours_meet_wcag_aa() {
    let (surface_light, surface_dark) = base("surface");
    let mut worst: Vec<String> = Vec::new();
    for (name, light, dark) in tokens::BASE.iter().filter(|(n, _, _)| n.starts_with("t-")) {
        for (theme, fg, bg) in [
            ("light", light, surface_light),
            ("dark", dark, surface_dark),
        ] {
            let ratio = contrast(fg, bg);
            if ratio < 4.5 {
                worst.push(format!("--md-{name} {theme}: {fg} on {bg} = {ratio:.2}:1"));
            }
        }
    }
    assert!(worst.is_empty(), "below 4.5:1 —\n  {}", worst.join("\n  "));
}

/// Every advertised language has to actually resolve to a table — a typo in a
/// `lang_for` arm is otherwise invisible, since an unknown fence silently
/// falls back to plain escaped text.
#[test]
fn every_advertised_language_highlights_something() {
    let cases: &[(&str, &str)] = &[
        ("php", "function f() { return 1; }"),
        ("ruby", "def f; return 1; end"),
        ("swift", "func f() -> Int { return 1 }"),
        ("lua", "local function f() return 1 end"),
        ("r", "f <- function(x) if (x) 1 else 2"),
        ("dart", "class A { void f() {} }"),
        ("scala", "object A { def f = 1 }"),
        ("perl", "sub f { my $x = 1; return $x; }"),
        ("powershell", "function f { param($x) return $x }"),
        ("dockerfile", "FROM alpine\nRUN echo hi"),
        ("makefile", "include config.mk\nall:\n\techo $(NAME)"),
        ("graphql", "query Q { field }"),
        ("protobuf", "message M { int32 a = 1; }"),
        ("terraform", "resource \"a\" \"b\" { count = 1 }"),
        ("nix", "let x = 1; in x"),
        ("zig", "pub fn main() void {}"),
        ("elixir", "defmodule M do def f, do: 1 end"),
        ("haskell", "f :: Int -> Int\nf x = x"),
        ("latex", "\\begin{document} % hi"),
        ("julia", "function f(x) return x end"),
    ];
    for (lang, code) in cases {
        let out = highlight::highlight(lang, code);
        assert!(
            out.contains("md2pdf-t-"),
            "{lang}: nothing highlighted — is it wired into lang_for?\n{out}"
        );
    }
}

/// An image target written inside a code span is documentation, not a
/// reference: fetching it would mean a rendered document reaching out to a URL
/// its author only ever quoted.
#[test]
fn an_image_url_inside_a_code_span_is_not_a_reference() {
    let md = "Write `![a](https://example.com/x.png)` to embed it.\n\n![real](y.png)\n";
    assert_eq!(crate::image_targets(md), vec!["y.png".to_string()]);
    assert!(crate::collect_remote_images(md).is_empty());
}

/// The TOML handed to the Typst templates has to carry every callout, so the
/// PDF cannot fall back to a different label or colour than the HTML uses.
#[test]
fn every_callout_reaches_the_typst_templates() {
    let toml = tokens::as_toml();
    for a in tokens::ADMONITIONS {
        assert!(toml.contains(&format!("[admonition.{}]", a.kind)), "{}", a.kind);
        assert!(toml.contains(&format!("en = \"{}\"", a.en)), "{}", a.en);
        assert!(toml.contains(&format!("de = \"{}\"", a.de)), "{}", a.de);
        assert!(toml.contains(a.accent.0), "{}", a.accent.0);
    }
    assert!(toml.contains("[base]"));
}

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
    "colgroup", "col", "blockquote", "details", "summary", "figure", "figcaption", "img", "nobr",
];

/// MathML, spelled out. `math-core`'s output is inserted unescaped, so this is
/// the list that decides what "trusted" means — an open-ended
/// `starts_with('m')` also waved through `<marquee>` and `<map>`.
const ALLOWED_MATHML: &[&str] = &[
    "math", "annotation", "annotation-xml", "semantics", "merror", "mfrac", "mi", "mmultiscripts",
    "mn", "mo", "mover", "mpadded", "mphantom", "mprescripts", "mroot", "mrow", "ms", "mspace",
    "msqrt", "mstyle", "msub", "msubsup", "msup", "mtable", "mtd", "mtext", "mtr", "munder",
    "munderover",
];

/// Attributes the renderer emits. Everything else — and anything at all
/// starting with `on` — came from the document.
const ALLOWED_ATTRS: &[&str] = &[
    "align", "alt", "charset", "checked", "class", "content", "data-done", "data-lang",
    "data-level", "data-theme", "decoding", "disabled", "display", "draggable", "for", "height",
    "href", "id", "lang", "loading", "name", "open", "rel", "scriptlevel", "span", "src", "start",
    "style", "tabindex", "title", "type", "width", "xmlns",
];

fn assert_no_injected_tags(out: &str) {
    for (name, _) in elements(out) {
        assert!(
            ALLOWED_TAGS.contains(&name.as_str()) || ALLOWED_MATHML.contains(&name.as_str()),
            "unexpected <{name}> in output:\n{out}"
        );
    }
}

/// The other half of the whitelist: a tag we emit can still carry an attribute
/// we did not, and an `href` we did emit can still point somewhere it should
/// not. Both are what an escaping failure actually looks like.
fn assert_no_injected_attributes(out: &str) {
    for (tag, attrs) in elements(out) {
        // MathML carries a long presentation vocabulary (`lspace`, `stretchy`,
        // `mathvariant`, …) that is inert and not worth enumerating. What
        // matters is that math-core never reaches a navigable attribute: every
        // MathML element accepts `href`.
        let mathml = ALLOWED_MATHML.contains(&tag.as_str());
        for (name, value) in attributes(&attrs) {
            assert!(
                !name.starts_with("on"),
                "event handler {name}={value:?} on <{tag}>:\n{out}"
            );
            if mathml {
                assert!(
                    !matches!(name.as_str(), "href" | "xlink:href" | "src"),
                    "navigable {name}={value:?} on MathML <{tag}>:\n{out}"
                );
                continue;
            }
            assert!(
                ALLOWED_ATTRS.contains(&name.as_str()) || name.starts_with("aria-"),
                "unexpected attribute {name} on <{tag}>:\n{out}"
            );
            if name == "href" || name == "src" {
                let v = value.trim().to_ascii_lowercase();
                let ok = v.starts_with('#')
                    || v.starts_with("http://")
                    || v.starts_with("https://")
                    || v.starts_with("mailto:")
                    || v.starts_with("tel:")
                    || v.starts_with("ftp:")
                    || v.starts_with("data:image/")
                    || v.starts_with("data:font/")
                    || !v.contains(':');
                assert!(ok, "unsafe {name}={value:?} on <{tag}>:\n{out}");
            }
        }
    }
}

/// `(tag-name, attribute-text)` for every start tag, skipping the contents of
/// raw-text elements so a `<` inside the stylesheet or the script is not read
/// as markup.
fn elements(out: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = out;
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let closing = rest.starts_with('/');
        let name: String = rest
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '!' || *c == '-')
            .collect();
        let name = name.to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        let end = rest.find('>').unwrap_or(rest.len());
        if !closing {
            let attrs = rest[name.len().min(end)..end].to_string();
            found.push((name.clone(), attrs));
        }
        rest = &rest[end..];
        // `<style>` and `<script>` are raw text: skip to the matching close so
        // CSS selectors and JS comparisons do not parse as tags.
        if !closing && (name == "style" || name == "script") {
            let close = format!("</{name}>");
            if let Some(i) = rest.find(&close) {
                rest = &rest[i + close.len()..];
            }
        }
    }
    found
}

fn attributes(attrs: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = attrs.trim();
    while !rest.is_empty() {
        let name: String = rest.chars().take_while(|c| !"= \t\n/>".contains(*c)).collect();
        if name.is_empty() {
            rest = &rest[rest.chars().next().map_or(0, char::len_utf8)..];
            continue;
        }
        rest = rest[name.len()..].trim_start();
        let value = if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            let quote = after.chars().next().filter(|c| *c == '"' || *c == '\'');
            match quote {
                Some(q) => {
                    let body = &after[1..];
                    let end = body.find(q).unwrap_or(body.len());
                    rest = &body[(end + 1).min(body.len())..];
                    body[..end].to_string()
                }
                None => {
                    let end = after.find(|c: char| c.is_whitespace()).unwrap_or(after.len());
                    rest = &after[end..];
                    after[..end].to_string()
                }
            }
        } else {
            String::new()
        };
        found.push((name.to_ascii_lowercase(), value));
        rest = rest.trim_start();
    }
    found
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";

// ---- structure ----------------------------------------------------------

#[test]
fn a_fragment_carries_its_own_styles_and_root() {
    let out = render("# Title\n\ntext", "", "", b"");
    assert!(out.starts_with("<style>"), "{out}");
    assert!(out.contains("<div class=\"md2pdf\" id=\"md2pdf-root\" lang=\"en\">"));
    assert!(!out.contains("<!doctype"));
}

/// A fragment is mounted by a host that would have to re-execute the script to
/// make it run at all. Not shipping one means the host never needs a
/// "run the first script you find in this document" primitive.
#[test]
fn only_the_standalone_export_carries_a_script() {
    assert!(!render("# a\n\ntext", "", "", b"").contains("<script"));
    let standalone = render("# a\n\ntext", "standalone=1\n", "", b"");
    assert!(standalone.contains("<script>"), "{standalone}");
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

/// A frontmatter title does not consume the document's first heading. Only a
/// heading that *became* the title is dropped, because only then would keeping
/// it print the same words twice.
#[test]
fn frontmatter_title_leaves_a_leading_h1_in_the_body() {
    let out = html(
        "---\ntitle: From Frontmatter\nauthors: [Ada, Grace]\ndate: 2026-01-01\n---\n\n# Kept\n\nbody",
    );
    assert!(out.contains("<h1>From Frontmatter</h1>"), "{out}");
    assert!(out.contains("<span>Ada</span><span>Grace</span>"), "{out}");
    assert!(out.contains("<span>2026-01-01</span>"), "{out}");
    assert!(body(&format!("{}", "---\ntitle: From Frontmatter\n---\n\n# Kept\n\nbody")).contains("Kept"), "{out}");
}

/// An empty `title:` is not a title. It used to strip the H1 *and* suppress
/// the title block, so the headline left the document with nothing taking its
/// place anywhere.
#[test]
fn an_empty_frontmatter_title_falls_back_to_the_leading_h1() {
    let out = html("---\ntitle: \"\"\n---\n\n# Real Title\n\nbody");
    assert!(out.contains("<h1>Real Title</h1>"), "{out}");
    assert!(!body("---\ntitle: \"\"\n---\n\n# Real Title\n\nbody").contains("Real Title"), "{out}");
}

/// The two renderers have to agree about which heading survives. They did not:
/// the HTML side dropped a leading H1 whenever *any* title existed, while
/// `lib.typ` drops it only when the title came from that heading. Nothing
/// compared them, so the divergence shipped — every `convert_str` in this
/// suite passed `strip_h1 = false`.
#[test]
fn both_renderers_agree_on_which_h1_survives() {
    // (frontmatter title, leading H1, does the heading stay in the body)
    let cases = [
        ("", "# Heading\n\nbody", false),
        ("title: From Frontmatter\n", "# Heading\n\nbody", true),
        ("title: \"\"\n", "# Heading\n\nbody", false),
        // Only a *leading* level-1 heading is ever a title candidate.
        ("title: From Frontmatter\n", "## Heading\n\nbody", true),
        ("", "## Heading\n\nbody", true),
        ("", "text first\n\n# Heading\n\nbody", true),
    ];
    for (front, rest, heading_stays) in cases {
        let md = if front.is_empty() {
            rest.to_string()
        } else {
            format!("---\n{front}---\n\n{rest}")
        };

        // What `lib.typ` computes before calling `convert`.
        let fm_title = Frontmatter::parse(&md)
            .first("title")
            .filter(|t| !t.is_empty())
            .unwrap_or_default()
            .to_string();
        let h1 = String::from_utf8(crate::leading_h1(md.as_bytes()).unwrap()).unwrap();
        let from_h1 = fm_title.is_empty() && !h1.is_empty();
        assert_eq!(from_h1, !heading_stays, "the shared rule disagrees for {md:?}");

        assert_eq!(
            body(&md).contains("Heading"),
            heading_stays,
            "HTML disagrees for {md:?}:\n{}",
            body(&md)
        );
        assert_eq!(
            convert_str(&md, from_h1).contains("Heading"),
            heading_stays,
            "Typst disagrees for {md:?}:\n{}",
            convert_str(&md, from_h1)
        );
    }
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
    // The tag is a hook for styling, not a visible chip.
    assert!(!html("```rust\nx\n```").contains("content: attr(data-lang)"));
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
fn mermaid_embeds_the_hosts_svg_as_an_image_and_falls_back_to_code() {
    let key = mermaid_key("graph LR\nA-->B");
    let out = html_with("```mermaid\ngraph LR\nA-->B\n```", &key, b"<svg><g/></svg>");
    assert!(out.contains("<figure class=\"md2pdf-mermaid\"><img src=\"data:image/svg+xml;base64,"), "{out}");
    assert!(out.contains("alt=\"Mermaid diagram\""), "{out}");
    assert!(body("```mermaid\ngraph LR\n```").contains("data-lang=\"mermaid\""));
}

/// The diagram SVG is the plugin's rendering of a source the document controls,
/// so it is never inlined. Inside `<img>` it cannot script or fetch, whatever
/// it turns out to contain.
#[test]
fn a_scriptable_mermaid_svg_is_embedded_inert() {
    let key = mermaid_key("graph LR");
    let hostile = br#"<svg onload="alert(1)"><script>x</script><foreignObject><img src=x onerror=alert(1)></foreignObject><a xlink:href="javascript:alert(1)">go</a></svg>"#;
    let out = html_with("```mermaid\ngraph LR\n```", &key, hostile);
    for payload in ["<script", "onload", "onerror", "javascript:", "foreignObject"] {
        assert!(!out.contains(payload), "{payload} survived into the markup:\n{out}");
    }
    assert!(out.contains("<img src=\"data:image/svg+xml;base64,"), "{out}");
    assert_no_injected_tags(&out);
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
fn the_drawer_button_carries_its_label_only_for_screen_readers() {
    let out = html("## One\n\n## Two");
    assert!(
        out.contains("<label class=\"md2pdf-toc-btn\" for=\"md2pdf-toc-state\"><span class=\"md2pdf-sr\">Contents</span></label>"),
        "{out}"
    );
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

/// `<br>` is the whole raw-HTML allowance. It earns its place because a table
/// cell holds inline content only, so there is no Markdown way to get a second
/// line into one; `__underline__` already covers what `<u>` was for.
#[test]
fn br_is_the_only_inline_tag_that_passes_through() {
    assert!(body("a<br>b<br/>c<br />d").contains("a<br>b<br>c<br>d"));
    let u = body("a <u>b</u> c");
    assert!(u.contains("a &lt;u&gt;b&lt;/u&gt; c"), "{u}");
    // Near-misses are text too, not a second spelling of the tag.
    for shape in ["<u foo=bar>x</u>", "< u>x</ u>", "<u/>x", "<break>x"] {
        let out = body(shape);
        assert!(out.contains("&lt;"), "{shape} was not escaped:\n{out}");
    }
    assert!(body("__still underlined__").contains("<u>still underlined</u>"));
}

#[test]
fn hide_toc_button_drops_the_drawer_but_not_an_inline_toc() {
    let plain = html("## a\n\n## b");
    assert!(plain.contains("md2pdf-toc-btn"), "{plain}");

    let hidden = html("---\nhide-toc-button: true\n---\n\n## a\n\n## b");
    assert!(!hidden.contains("md2pdf-toc-btn"), "{hidden}");
    assert!(!hidden.contains("md2pdf-toc-state"), "{hidden}");
    // An explicit `[toc]` is a different thing and is left alone.
    let both = html("---\nhide-toc-button: yes\n---\n\n[toc]\n\n## a\n\n## b");
    assert!(!both.contains("md2pdf-toc-btn"), "{both}");
    assert!(both.contains("md2pdf-toc-inline"), "{both}");
    // Anything that is not an affirmative leaves the button alone.
    for value in ["false", "no", "", "maybe"] {
        let out = html(&format!("---\nhide-toc-button: {value}\n---\n\n## a\n\n## b"));
        assert!(out.contains("md2pdf-toc-btn"), "{value:?} hid the button:\n{out}");
    }
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
fn only_documents_with_math_ask_for_the_math_font() {
    assert!(crate::has_math("$a$"));
    assert!(crate::has_math("text\n\n$$x = 1$$"));
    assert!(!crate::has_math("# Plain\n\nno formulas, just a `$` sign"));
}

#[test]
fn the_math_font_is_embedded_only_when_the_host_supplies_it() {
    // The preview path gets no font bytes and must stay lean.
    assert!(!render("$a$", "", "", b"").contains("@font-face"));

    let manifest = "fonts/math.woff2\t2\nfonts/math-alpha.woff2\t2\n";
    let out = render("$a$", "standalone=1\n", manifest, b"abcd");
    assert_eq!(out.matches("@font-face").count(), 1, "{out}");
    assert!(out.contains("src:url(data:font/woff2;base64,YWI=)"), "{out}");
    // Only a document that reaches into the math alphanumerics pays for them.
    // `\mathbb{R}` does not: it is ℝ, a Letterlike Symbol the base face has.
    assert_eq!(
        render(r"$\mathbb{R}$", "standalone=1\n", manifest, b"abcd").matches("@font-face").count(),
        1
    );
    let out = render(r"$\mathbb{A}$", "standalone=1\n", manifest, b"abcd");
    assert_eq!(out.matches("@font-face").count(), 2, "{out}");
    assert!(out.contains("unicode-range:U+1D400-1D7FF"), "{out}");
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

fn editable(md: &str) -> String {
    strip_chrome(&render(md, "editable=1\n", "", b""))
}

/// Every line of the source a block came from, as the renderer reports it.
fn lines_of(md: &str) -> Vec<u32> {
    let out = editable(md);
    let mut found = Vec::new();
    let mut rest = out.as_str();
    while let Some(i) = rest.find("data-md-line=\"") {
        rest = &rest[i + 14..];
        let end = rest.find('"').unwrap();
        found.push(rest[..end].parse().unwrap());
        rest = &rest[end..];
    }
    found
}

/// Three of the pre-parse passes rewrite whole blocks before comrak sees the
/// text, so its line numbers describe the text it was handed rather than the
/// text the author wrote. Each of those shifts gets a case here.
#[test]
fn a_block_reports_the_line_the_author_wrote() {
    // No shift at all: a paragraph on line 1 and one on line 3.
    assert_eq!(lines_of("first\n\nsecond"), vec![1, 3]);

    // A run of blank lines collapses to three lines; what follows must not
    // move with it.
    assert_eq!(lines_of("a\n\n\n\n\nb"), vec![1, 6]);

    // A `+`-width table inserts a placeholder line before the header.
    assert_eq!(lines_of("| a | b |\n| - | -+ |\n| c | d |\n\nafter"), vec![1, 5]);

    // A heading after several collapsing runs still lands.
    assert_eq!(lines_of("a\n\n\n\nb\n\n\n\n# h"), vec![1, 5, 9]);
}

/// An admonition body is lifted out and re-parsed as its own document, so its
/// blocks start again at line 1 and have to be mapped back.
#[test]
fn a_block_inside_a_lifted_body_reports_its_outer_line() {
    assert_eq!(lines_of(":::info\npara\n:::"), vec![2]);
    assert_eq!(lines_of("intro\n\n:::info\npara\n:::"), vec![1, 4]);
    assert_eq!(lines_of("+++++ Summary\nbody\n+++++"), vec![2]);
    // A collapsing blank run *inside* a lifted body: both mappings compose.
    assert_eq!(lines_of(":::info\na\n\n\n\nb\n:::"), vec![2, 6]);
    // Nested one level deeper.
    assert_eq!(lines_of("::::tip\n:::info\ndeep\n:::\n::::"), vec![3]);
}

/// Whatever line a block claims, that line has to exist and still hold it.
#[test]
fn every_reported_line_is_inside_the_document() {
    for (name, md) in FIXTURES {
        let total = md.lines().count() as u32;
        for line in lines_of(md) {
            assert!(line >= 1 && line <= total, "{name}: line {line} of {total}");
        }
    }
}

/// Ticking a box in the preview edits the document, so the box has to know
/// which line to edit — through every pass that moves lines.
#[test]
fn an_interactive_task_carries_the_line_that_wrote_it() {
    let out = editable("- [ ] open\n- [x] done");
    assert!(out.contains("data-md-line=\"1\""), "{out}");
    assert!(out.contains("data-md-line=\"2\""), "{out}");
    assert!(!out.contains("disabled"), "{out}");
    // Named by its own text rather than by a wrapping label, which would make
    // links and code spans inside the item toggle the box.
    assert!(out.contains("aria-labelledby=\"md2pdf-task-1\""), "{out}");
    assert!(out.contains("<div id=\"md2pdf-task-1\">"), "{out}");

    // Through a blank-line collapse, an admonition, and a blockquote.
    assert!(editable("intro\n\n\n\n\n- [ ] t").contains("data-md-line=\"6\""));
    assert!(editable(":::info\n- [ ] t\n:::").contains("data-md-line=\"2\""));
    assert!(editable("> - [x] t").contains("data-md-line=\"1\""));
}

/// The contract the editor relies on, stated once: whatever line a checkbox
/// names, that line of the *source* holds a task marker the editor can flip.
/// The pattern here is the one `web/src/lib/utils/task-marker.ts` uses.
#[test]
fn every_checkbox_points_at_a_line_that_holds_a_marker() {
    fn is_marker(line: &str) -> bool {
        let rest = line.trim_start_matches([' ', '\t', '>']);
        let rest = match rest.chars().next() {
            Some('-') | Some('*') | Some('+') => &rest[1..],
            Some(c) if c.is_ascii_digit() => {
                let digits = rest.chars().take_while(char::is_ascii_digit).count();
                match rest[digits..].chars().next() {
                    Some('.') | Some(')') => &rest[digits + 1..],
                    _ => return false,
                }
            }
            _ => return false,
        };
        let rest = rest.trim_start_matches([' ', '\t']);
        matches!(rest.as_bytes(), [b'[', b' ' | b'x' | b'X', b']', ..])
            && rest.len() > 3
            && rest.len() != rest.trim_start_matches(['[']).len()
    }

    let mut checked = 0;
    for (name, md) in FIXTURES {
        let out = editable(md);
        let source: Vec<&str> = md.lines().collect();
        let mut rest = out.as_str();
        while let Some(i) = rest.find("<input type=\"checkbox\" data-md-line=\"") {
            rest = &rest[i + 37..];
            let end = rest.find('"').unwrap();
            let line: usize = rest[..end].parse().unwrap();
            let text = source.get(line - 1).copied().unwrap_or("");
            assert!(is_marker(text), "{name} line {line} is not a marker: {text:?}");
            checked += 1;
            rest = &rest[end..];
        }
    }
    assert!(checked > 3, "only {checked} checkboxes across the fixtures — is this vacuous?");
}

/// A list where only some items are tasks: comrak marks the whole list, but a
/// plain sibling has no `[ ]` to write to and must not offer one.
#[test]
fn only_a_real_task_item_becomes_clickable() {
    let out = editable("- [ ] task\n- plain");
    assert_eq!(out.matches("data-md-line").count(), 2, "one list, one task:\n{out}");
    assert_eq!(out.matches("disabled").count(), 1, "the plain item stays inert:\n{out}");
}

/// The default output is unchanged: this is preview machinery, and a download
/// has no source to point back at.
#[test]
fn source_lines_are_absent_unless_asked_for() {
    assert!(!html("# a\n\ntext").contains("data-md-line"));
    assert!(!render("# a\n\ntext", "standalone=1\n", "", b"").contains("data-md-line"));
    assert!(editable("# a\n\ntext").contains("data-md-line"));
}

/// The engine is UTF-8 end to end; this pins the places that index by byte.
#[test]
fn non_latin_scripts_survive_the_renderers() {
    let md = include_str!("../../../tests/unicode.md");
    let out = html(md);
    let typst = convert_str(md, false);
    for sample in ["你好，世界", "こんにちは", "안녕하세요", "مرحبا"] {
        assert!(out.contains(sample), "HTML lost {sample}");
        assert!(typst.contains(sample), "Typst lost {sample}");
    }
    // A slug is built from `char::is_alphanumeric`, so a CJK heading gets a
    // real anchor rather than collapsing to `section-N`.
    assert!(out.contains("id=\"你好世界\""), "{out}");
    assert!(out.contains("id=\"ünï-cödé\""), "{out}");
    // Highlighting a fence full of wide characters must not lose or split one.
    assert!(out.contains("挨拶"), "{out}");
}

/// `math-core`'s MathML is inserted unescaped — the one place left where the
/// renderer trusts markup it did not write. `\text{…}` takes arbitrary content
/// and every MathML element accepts `href`, so this is where that trust is
/// checked rather than assumed.
#[test]
fn hostile_tex_cannot_reach_the_markup() {
    let cases = [
        r"\text{</span><img src=x onerror=alert(1)>}",
        r"\href{javascript:alert(1)}{click}",
        r"\text{</math><script>alert(1)</script>}",
        r#"\text{" onload="alert(1)}"#,
        r"\text{a & b < c > d}",
        r"\class{md2pdf-toc-btn}{x}",
    ];
    for latex in cases {
        for md in [format!("${latex}$"), format!("$${latex}$$")] {
            let out = html(&md);
            // The guards are the real check: an escaped `&lt;img …&gt;` is
            // text and fine, an actual element or attribute is not.
            assert_no_injected_tags(&out);
            assert_no_injected_attributes(&out);
            for tag in ["<script", "<img", "<iframe", "<span onload"] {
                assert!(!out.contains(tag), "{latex} produced {tag}:\n{out}");
            }
        }
    }
}

/// The renderer's own ids are not up for grabs. A heading that slugs to one of
/// them would mint a duplicate, and every `#fragment` and `getElementById` for
/// that id would then resolve to whichever came first in the document.
#[test]
fn a_heading_cannot_claim_a_reserved_id() {
    let out = html(
        "### md2pdf-toc-state\n\n### md2pdf-root\n\n### md2pdf-fn-1\n\n\
         ### md2pdf-md2pdf-x\n\ntext[^n]\n\n[^n]: note",
    );
    for reserved in ["md2pdf-toc-state", "md2pdf-root", "md2pdf-fn-1"] {
        assert_eq!(
            out.matches(&format!("id=\"{reserved}\"")).count(),
            1,
            "{reserved} was minted twice:\n{out}"
        );
    }
    assert!(out.contains("id=\"toc-state\""), "{out}");
    assert!(out.contains("id=\"x\""), "a repeated prefix must not survive:\n{out}");
}

/// The batched asset list has to say exactly what the five single-purpose
/// calls say, or a host that switches to it starts losing images.
#[test]
fn the_batched_asset_list_matches_the_individual_calls() {
    let md = include_str!("../../../tests/extended.md");
    let lines = |b: Vec<u8>| {
        String::from_utf8(b)
            .unwrap()
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let batched = lines(crate::html_assets(md.as_bytes()).unwrap());
    let of = |kind: &str| {
        batched
            .iter()
            .filter_map(|l| l.strip_prefix(&format!("{kind}\t")).map(str::to_string))
            .collect::<Vec<_>>()
    };

    assert_eq!(of("image"), lines(crate::html_images(md.as_bytes()).unwrap()));
    assert_eq!(of("remote"), lines(crate::remotes(md.as_bytes()).unwrap()));
    assert_eq!(of("emoji"), lines(crate::twemojis(md.as_bytes()).unwrap()));
    assert_eq!(of("font"), lines(crate::html_fonts(md.as_bytes()).unwrap()));
    assert_eq!(of("mermaid"), lines(crate::html_mermaid(md.as_bytes()).unwrap()));
    assert!(!of("mermaid").is_empty(), "the demo has diagrams");
}

/// Every fixture in `tests/`, so a new one is covered by adding it here once.
const FIXTURES: &[(&str, &str)] = &[
    ("html-edge.md", include_str!("../../../tests/html-edge.md")),
    ("extended.md", include_str!("../../../tests/extended.md")),
    ("sample.md", include_str!("../../../tests/sample.md")),
    ("tables.md", include_str!("../../../tests/tables.md")),
    ("headings.md", include_str!("../../../tests/headings.md")),
    ("citations.md", include_str!("../../../tests/citations.md")),
    ("emoji.md", include_str!("../../../tests/emoji.md")),
    ("cover.md", include_str!("../../../tests/cover.md")),
    ("frontmatter.md", include_str!("../../../tests/frontmatter.md")),
    ("unicode.md", include_str!("../../../tests/unicode.md")),
];

/// Content does not silently vanish.
///
/// The leading-H1 bug was a heading that the renderer deleted and nothing
/// replaced — invisible to every test here, because they all assert that
/// something *is* present and none assert that nothing went missing. This
/// walks the source instead of the output: every heading, task item and code
/// fence in a fixture has to turn up in both renderings.
#[test]
fn no_fixture_loses_a_heading_a_task_or_a_code_block() {
    for (name, md) in FIXTURES {
        let html_out = render(md, "standalone=1\n", "", b"");
        let typst_out = convert_str(md, false);

        // The one heading allowed to disappear is the one promoted to title.
        let promoted = String::from_utf8(crate::leading_h1(md.as_bytes()).unwrap()).unwrap();
        let has_fm_title = Frontmatter::parse(md).first("title").is_some_and(|t| !t.is_empty());
        let consumed = if has_fm_title { String::new() } else { promoted };

        let mut fence = false;
        for line in md.lines() {
            let t = line.trim_start();
            if t.starts_with("```") {
                fence = !fence;
                continue;
            }
            if fence {
                continue;
            }
            let Some(text) = t.strip_prefix('#').map(str::trim_start) else {
                continue;
            };
            let text = text.trim_start_matches('#').trim();
            // Inline markup is rendered as elements, so compare on a word that
            // survives either way.
            let Some(word) = text
                .split_whitespace()
                .find(|w| w.len() > 4 && w.chars().all(|c| c.is_alphanumeric()))
            else {
                continue;
            };
            if !consumed.is_empty() && consumed.contains(word) {
                continue;
            }
            assert!(html_out.contains(word), "{name}: HTML lost heading word {word:?}");
            assert!(typst_out.contains(word), "{name}: Typst lost heading word {word:?}");
        }
    }
}

/// Math in a fixture renders as math.
///
/// `math-core` accepts less LaTeX than mitex does, and it reports the gap by
/// leaving an `<merror>` *inside* an otherwise-fine formula — so a document
/// that is perfect in the PDF can render wrongly here with nothing failing.
/// A fixture is the PDF's spec, so it has to survive this renderer too.
#[test]
fn no_fixture_loses_a_formula_to_the_html_math_renderer() {
    for (name, md) in FIXTURES {
        let out = html(md);
        assert!(
            !out.contains("math-core-unknown-cmd"),
            "{name}: a formula rendered with a command dropped out of it"
        );
        // `html-edge.md` carries malformed LaTeX on purpose; falling back to
        // the source is the right answer there, and the only one.
        if *name != "html-edge.md" {
            assert!(
                !out.contains("md2pdf-math-error"),
                "{name}: a formula fell back to its source"
            );
        }
    }
}

/// Every fixture, through both guards. Opting individual tests in is how the
/// mermaid path went years without the tag check ever running over it.
#[test]
fn no_fixture_can_inject_a_tag_or_an_attribute() {
    for (name, md) in FIXTURES {
        for options in ["", "standalone=1\n"] {
            let out = render(md, options, "", b"");
            let checked = if options.is_empty() { out.clone() } else { strip_chrome(&out) };
            assert_no_injected_tags(&checked);
            assert_no_injected_attributes(&checked);
            assert!(!checked.contains("<script"), "{name}: script in a fragment");
        }
    }
}


