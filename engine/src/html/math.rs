//! LaTeX -> MathML. The PDF path delegates math to mitex inside Typst; HTML
//! needs real markup, and MathML Core renders natively in every current
//! browser — no JS, no webfont, sharp at any zoom, and it inherits the
//! surrounding colour so both themes work for free.
//!
//! The two libraries do not accept the same LaTeX. mitex covers most of
//! amsmath; `math-core` covers less, and with `ignore_unknown_commands` it
//! drops what it does not know into an inline `<merror>` rather than failing —
//! so a formula that is perfect in the PDF used to come out of the web preview
//! as an error blob followed by a half-rendered formula. `rewrite` closes that
//! gap for the commands that have a faithful equivalent, and anything left
//! over is reported instead of quietly rendering wrong.

use math_core::{LatexToMathML, MathCoreConfig, MathDisplay};

/// The marker `math-core` puts on a command it did not recognise.
const UNKNOWN: &str = "math-core-unknown-cmd";

/// Commands that differ only in name.
///
/// `\url` loses its link, which is the point: MathML elements accept `href`,
/// and a document is untrusted input.
const RENAMED: &[(&str, &str)] = &[
    ("widetilde", "tilde"),
    ("mbox", "text"),
    ("hbox", "text"),
    ("url", "text"),
];

/// Commands that only adjust spacing or layout. MathML positions these itself,
/// so dropping the name and keeping the argument loses nothing visible.
const DROPPED: &[&str] = &["smash", "limits", "nolimits", "displaystyle", "textstyle"];

/// Environments that differ only in name. `smallmatrix` renders full size and
/// `alignedat` loses its column count — both are closer than an error.
const ENVIRONMENTS: &[(&str, &str)] = &[
    ("split", "aligned"),
    ("smallmatrix", "matrix"),
    ("alignedat", "aligned"),
];

/// `\boxed` and its aliases. `math-core` has no boxing primitive at all — not
/// `\fbox`, not `\enclose`, not `\bbox` — so the border is drawn in CSS around
/// the whole formula instead.
const BOXES: &[&str] = &["boxed", "fbox", "framebox"];

pub(crate) struct Math(LatexToMathML);

impl Math {
    pub(crate) fn new() -> Self {
        let config = MathCoreConfig {
            // A typo in one formula must not blank the whole document.
            ignore_unknown_commands: true,
            ..Default::default()
        };
        // `new` only fails on malformed custom macros, and we define none.
        Self(LatexToMathML::new(config).expect("no custom macros"))
    }

    /// Render one formula. On a parse error, or a command with no MathML
    /// equivalent, the LaTeX source is shown verbatim so the author can see
    /// what to fix, matching how Typst surfaces bad math.
    pub(crate) fn render(&self, display: bool, latex: &str) -> String {
        let source = latex.trim();
        let (body, boxed) = unwrap_box(source);
        let mode = if display {
            MathDisplay::Block
        } else {
            MathDisplay::Inline
        };
        let mut class = String::from("md2pdf-math");
        if display {
            class.push_str(" md2pdf-math-block");
        }
        if boxed {
            class.push_str(" md2pdf-math-boxed");
        }
        match self.0.convert_with_local_state(&rewrite(&body), mode) {
            Ok(result) if !result.mathml.contains(UNKNOWN) => {
                format!("<span class=\"{class}\">{}</span>", result.mathml)
            }
            // An `<merror>` left in the markup means the formula rendered
            // *wrongly* — the command was dropped, not skipped. Better to show
            // the source and name the command than to draw the wrong thing.
            Ok(result) => error(source, &unknown_command(&result.mathml)),
            Err(err) => error(source, &err.to_string()),
        }
    }
}

fn error(source: &str, why: &str) -> String {
    format!(
        "<code class=\"md2pdf-math-error\" title=\"{}\">{}</code>",
        super::esc_attr(why),
        super::esc_text(source)
    )
}

/// Name the command `math-core` choked on, for the tooltip.
fn unknown_command(mathml: &str) -> String {
    let named = mathml
        .split_once(UNKNOWN)
        .and_then(|(_, rest)| rest.split_once("<mtext>"))
        .and_then(|(_, rest)| rest.split_once("</mtext>"))
        .map(|(name, _)| name.trim());
    match named {
        Some(name) if !name.is_empty() => format!("{name} has no MathML equivalent"),
        _ => "unsupported command".to_string(),
    }
}

/// Strip a `\boxed{…}` that wraps the whole formula, reporting that it did.
///
/// Only the whole-formula case: a box around part of a formula cannot be drawn
/// by putting a border on the element holding all of it, and rendering it
/// unboxed would silently lose the emphasis the author asked for. That case
/// falls through to the error path instead.
fn unwrap_box(latex: &str) -> (String, bool) {
    for name in BOXES {
        let Some(rest) = latex.strip_prefix(&format!("\\{name}")) else {
            continue;
        };
        let rest = rest.trim_start();
        let mut at = 0;
        let src: Vec<char> = rest.chars().collect();
        if src.first() != Some(&'{') {
            continue;
        }
        let inner = read_group(&src, &mut at);
        if at == src.len() {
            return (inner, true);
        }
    }
    (latex.to_string(), false)
}

/// Read the brace group starting at `i`, leaving `i` just past its `}`.
fn read_group(src: &[char], i: &mut usize) -> String {
    if src.get(*i) != Some(&'{') {
        return String::new();
    }
    let mut depth = 0;
    let start = *i + 1;
    while *i < src.len() {
        match src[*i] {
            // A brace the author escaped is content, not structure.
            '\\' => *i += 1,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let out = src[start..*i].iter().collect();
                    *i += 1;
                    return out;
                }
            }
            _ => {}
        }
        *i += 1;
    }
    src[start..].iter().collect()
}

/// Rewrite the commands `math-core` does not know into ones it does.
fn rewrite(latex: &str) -> String {
    let src: Vec<char> = latex.chars().collect();
    let mut out = String::with_capacity(latex.len());
    let mut i = 0;

    while i < src.len() {
        if src[i] != '\\' {
            out.push(src[i]);
            i += 1;
            continue;
        }

        let start = i + 1;
        let mut end = start;
        while end < src.len() && src[end].is_ascii_alphabetic() {
            end += 1;
        }
        // `\\`, `\;`, `\,` and friends carry no name — copy the pair through.
        if end == start {
            out.push('\\');
            if let Some(c) = src.get(start) {
                out.push(*c);
            }
            i = start + 1;
            continue;
        }

        let name: String = src[start..end].iter().collect();
        i = end;

        match name.as_str() {
            "href" => {
                // Drop the URL, keep the label.
                skip_space(&src, &mut i);
                read_group(&src, &mut i);
            }
            // `\color` is a switch that runs to the end of its group, while
            // `\textcolor` colours its argument alone — so the replacement has
            // to bring a group with it or the rest of the formula turns red.
            "textcolor" => {
                skip_space(&src, &mut i);
                let colour = read_group(&src, &mut i);
                skip_space(&src, &mut i);
                let body = read_group(&src, &mut i);
                out.push_str(&format!("{{\\color{{{colour}}}{{{}}}}}", rewrite(&body)));
            }
            "substack" => {
                skip_space(&src, &mut i);
                let body = read_group(&src, &mut i);
                out.push('{');
                out.push_str(&rewrite(&body.replace("\\\\", "\\atop ")));
                out.push('}');
            }
            "begin" | "end" => {
                skip_space(&src, &mut i);
                let env = read_group(&src, &mut i);
                let mapped = ENVIRONMENTS
                    .iter()
                    .find(|(from, _)| *from == env)
                    .map(|(_, to)| *to);
                out.push_str(&format!("\\{name}{{{}}}", mapped.unwrap_or(&env)));
                // `alignedat` takes a column count its replacement does not.
                if env == "alignedat" && name == "begin" {
                    skip_space(&src, &mut i);
                    read_group(&src, &mut i);
                }
            }
            _ if DROPPED.contains(&name.as_str()) => {}
            _ => {
                let to = RENAMED
                    .iter()
                    .find(|(from, _)| *from == name)
                    .map(|(_, to)| *to)
                    .unwrap_or(&name);
                out.push_str(&format!("\\{to}"));
            }
        }
    }
    out
}

fn skip_space(src: &[char], i: &mut usize) {
    while src.get(*i).is_some_and(|c| c.is_whitespace()) {
        *i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mathml(latex: &str) -> String {
        Math::new().render(true, latex)
    }

    #[test]
    fn renders_mathml() {
        let out = Math::new().render(false, r"\frac{a}{b}");
        assert!(out.contains("<math"), "{out}");
        assert!(out.contains("<mfrac>"), "{out}");
    }

    #[test]
    fn block_math_is_marked_as_display() {
        let out = Math::new().render(true, "x = 1");
        assert!(out.contains("md2pdf-math-block"), "{out}");
        assert!(out.contains("display=\"block\""), "{out}");
    }

    #[test]
    fn broken_latex_falls_back_to_escaped_source() {
        let out = Math::new().render(false, r"\frac{a");
        assert!(out.contains("md2pdf-math-error"), "{out}");
        assert!(out.contains(r"\frac{a"), "{out}");
    }

    #[test]
    fn math_metacharacters_are_escaped() {
        let out = Math::new().render(false, "a < b");
        assert!(!out.contains("<mo>&<"), "{out}");
        assert!(out.contains("&lt;") || out.contains("&#x3C;"), "{out}");
    }

    /// The formula that started this: perfect in the PDF, an error blob in the
    /// web preview.
    #[test]
    fn a_boxed_formula_renders_as_a_box() {
        let out = mathml(r"\boxed{X\to Y\to Z \;\Rightarrow\; I(X;Y)\ \ge\ I(X;Z)}");
        assert!(out.contains("md2pdf-math-boxed"), "{out}");
        assert!(!out.contains(UNKNOWN), "{out}");
        assert!(!out.contains("md2pdf-math-error"), "{out}");
        // The contents still render, rather than the box eating them.
        assert!(out.contains("⇒"), "{out}");
    }

    #[test]
    fn every_box_alias_is_a_box() {
        for name in BOXES {
            let out = mathml(&format!("\\{name}{{x}}"));
            assert!(out.contains("md2pdf-math-boxed"), "{name}: {out}");
        }
    }

    /// A box around part of a formula cannot be drawn on the element holding
    /// all of it, so it must not silently render unboxed.
    #[test]
    fn a_partial_box_is_reported_rather_than_dropped() {
        let out = mathml(r"a + \boxed{b} + c");
        assert!(!out.contains("md2pdf-math-boxed"), "{out}");
        assert!(out.contains("md2pdf-math-error"), "{out}");
        assert!(out.contains("boxed"), "{out}");
    }

    /// Every one of these renders in the PDF, so it has to render here.
    #[test]
    fn latex_the_pdf_accepts_does_not_become_an_error() {
        let m = Math::new();
        let cases = [
            r"\textcolor{red}{x}",
            r"\widetilde{xy}",
            r"\mbox{hi}",
            r"\hbox{hi}",
            r"\url{https://example.com}",
            r"\smash{x}",
            r"\sum\limits_{i=1}^n i",
            r"\sum\nolimits_i",
            r"\displaystyle\frac{a}{b}",
            r"\href{https://example.com}{label}",
            r"\substack{a\\b}",
            r"\begin{split} a &= b \\ c &= d \end{split}",
            r"\begin{smallmatrix} a & b \end{smallmatrix}",
            r"\begin{alignedat}{2} a &= b \end{alignedat}",
            r"\boxed{E = mc^2}",
        ];
        for case in cases {
            let out = m.render(true, case);
            assert!(!out.contains(UNKNOWN), "{case}\n{out}");
            assert!(!out.contains("md2pdf-math-error"), "{case}\n{out}");
            assert!(out.contains("<math"), "{case}\n{out}");
        }
    }

    /// The rewrite matches whole command names. `\text` is a prefix of
    /// `\textcolor`, so a substring replacement would corrupt one of them.
    #[test]
    fn rewriting_does_not_run_into_a_longer_command() {
        assert_eq!(
            rewrite(r"\text{a}\textcolor{red}{b}"),
            r"\text{a}{\color{red}{b}}"
        );
        assert_eq!(rewrite(r"\tilde{x}\widetilde{y}"), r"\tilde{x}\tilde{y}");
    }

    /// `\color` runs to the end of its group; `\textcolor` does not. Without
    /// the wrapping group the substitution reddens the rest of the formula.
    #[test]
    fn a_coloured_argument_does_not_colour_what_follows() {
        assert_eq!(rewrite(r"\textcolor{red}{x} + y"), r"{\color{red}{x}} + y");
    }

    /// Spacing commands have no name, so the name scanner must not eat the
    /// character after the backslash.
    #[test]
    fn unnamed_commands_survive_the_rewrite() {
        assert_eq!(rewrite(r"a \; b \, c \\ d \! e"), r"a \; b \, c \\ d \! e");
    }

    #[test]
    fn a_url_keeps_its_text_but_loses_its_link() {
        let out = mathml(r"\href{javascript:alert(1)}{click}");
        assert!(!out.contains("href"), "{out}");
        assert!(!out.contains("javascript"), "{out}");
        assert!(out.contains("click") || out.contains("<mi>c</mi>"), "{out}");
    }

    /// Nothing math-core emits may carry a link: it is third-party markup that
    /// the renderer inserts unescaped.
    #[test]
    fn no_formula_can_emit_a_link() {
        let m = Math::new();
        for case in [
            r"\href{https://example.com}{x}",
            r"\url{https://example.com}",
            r"\text{\href{https://example.com}{x}}",
        ] {
            let out = m.render(true, case);
            assert!(!out.contains("href="), "{case}\n{out}");
        }
    }

    #[test]
    fn a_group_with_nested_braces_is_read_whole() {
        let (inner, boxed) = unwrap_box(r"\boxed{\frac{a}{b}}");
        assert!(boxed);
        assert_eq!(inner, r"\frac{a}{b}");
    }

    #[test]
    fn an_unsupported_command_names_itself_in_the_tooltip() {
        let out = mathml(r"\rule{1em}{1pt}");
        assert!(out.contains("md2pdf-math-error"), "{out}");
        assert!(out.contains("rule"), "{out}");
    }
}
