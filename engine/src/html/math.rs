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

/// Commands and environments the PDF's LaTeX converter (mitex) rejects, which
/// abort the whole Typst compile. math-core is no judge of these: it accepts
/// some of them (`\href`, after `rewrite`) and knows others mitex does not.
const PDF_UNSUPPORTED: &[&str] = &[
    "Bbbk",
    "DeclareMathOperator",
    "SI",
    "ce",
    "char",
    "def",
    "enclose",
    "eqnarray",
    "eqref",
    "gather*",
    "genfrac",
    "href",
    "idotsint",
    "iiiint",
    "label",
    "lmoustache",
    "mathchoice",
    "mathds",
    "mbox",
    "multline",
    "phase",
    "prescript",
    "qty",
    "ref",
    "rule",
    "shortmid",
    "sideset",
    "slash",
    "strut",
    "url",
    "varinjlim",
    "varliminf",
    "vcenter",
    "verb",
];

/// Commands mitex renders that math-core reports as unknown.
const PDF_ONLY: &[&str] = &[
    "LaTeX",
    "TeX",
    "boxed",
    "centerdot",
    "colorbox",
    "dotsb",
    "dotsc",
    "fbox",
    "framebox",
    "hline",
    "mathnormal",
    "newcommand",
    "pmb",
    "pod",
    "raisebox",
    "smallsetminus",
    "tag",
    "textnormal",
    "thickapprox",
    "thicksim",
];

pub(crate) struct Math(LatexToMathML);

impl Math {
    pub(crate) fn new() -> Self {
        let config = MathCoreConfig {
            // A typo in one formula must not blank the whole document.
            ignore_unknown_commands: true,
            ..Default::default()
        };
        // `new` only fails on malformed custom macros, and we define none.
        Self(LatexToMathML::new(config, Vec::new()).expect("no custom macros"))
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
            // Browsers do not draw `<menclose>` (it is not in MathML Core), so
            // a box around part of a formula would silently vanish.
            Ok(result) if result.mathml.contains("<menclose") => error(
                source,
                "a box around part of a formula is not supported here",
            ),
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

impl Math {
    /// Whether the PDF can typeset this formula. mitex runs inside the Typst
    /// compile, where a failure cannot be caught and takes the whole document
    /// with it, so the engine decides up front and shows the source instead.
    pub(crate) fn pdf_accepts(&self, latex: &str) -> bool {
        let source = latex.trim();
        if source.is_empty() {
            return true;
        }
        if unclosed_label_in_text(source) || !mitex_structure_ok(source) {
            return false;
        }
        if commands(source).any(|c| PDF_UNSUPPORTED.contains(&c.as_str())) {
            return false;
        }
        match self
            .0
            .convert_with_local_state(&rewrite(source), MathDisplay::Block)
        {
            Ok(result) => unknown_commands(&result.mathml)
                .iter()
                .all(|c| PDF_ONLY.contains(&c.as_str())),
            // math-core is stricter about structure than mitex, which closes
            // open groups at the end and tolerates a dangling `^` or `&`; the
            // structural cases mitex does reject were checked above.
            Err(_) => true,
        }
    }
}

/// The structural mistakes mitex cannot recover from: a `}` that closes
/// nothing, an unpaired `\left`/`\right`, a fraction short of an argument, an
/// infix `\over` with nothing after it, an `\end` with no `\begin`, and a
/// style or limits switch with nothing to apply to.
fn mitex_structure_ok(latex: &str) -> bool {
    let src: Vec<char> = latex.chars().collect();
    let mut depth = 0i32;
    let mut i = 0;
    while i < src.len() {
        match src[i] {
            '\\' => i += 1,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let names: Vec<String> = commands(latex).collect();
    let count = |n: &str| names.iter().filter(|c| *c == n).count();
    if count("left") != count("right") {
        return false;
    }
    if latex.starts_with("\\limits") || latex.starts_with("\\nolimits") {
        return false;
    }
    let switches = [
        "displaystyle",
        "textstyle",
        "scriptstyle",
        "scriptscriptstyle",
    ];
    let mut words = latex.split('\\').map(str::trim).filter(|w| !w.is_empty());
    let only_switches = words.clone().next().is_some() && words.all(|w| switches.contains(&w));
    if only_switches {
        return false;
    }
    if names
        .iter()
        .any(|n| matches!(n.as_str(), "over" | "atop" | "choose"))
        && split_infix(latex)[1..].iter().any(|side| {
            side.trim_matches(|c: char| c.is_whitespace() || c == '}')
                .is_empty()
        })
    {
        return false;
    }
    if latex.matches("\\end{").count() > latex.matches("\\begin{").count() {
        return false;
    }
    fraction_arguments_ok(&src)
}

/// The operands around every `\over`, `\atop` and `\choose`. Only an empty
/// right-hand side is fatal; mitex supplies an empty left one itself.
fn split_infix(latex: &str) -> Vec<&str> {
    let mut sides = Vec::new();
    let mut from = 0;
    let mut i = 0;
    while let Some(at) = latex[i..].find('\\').map(|a| a + i) {
        let name_end = latex[at + 1..]
            .find(|c: char| !c.is_ascii_alphabetic())
            .map_or(latex.len(), |e| at + 1 + e);
        if matches!(&latex[at + 1..name_end], "over" | "atop" | "choose") {
            sides.push(&latex[from..at]);
            from = name_end;
        }
        i = name_end.max(at + 2).min(latex.len());
    }
    sides.push(&latex[from..]);
    sides
}

/// Every `\frac`-like command has two arguments. An unterminated group at the
/// end still counts as the second one — mitex closes it — but not as the first.
fn fraction_arguments_ok(src: &[char]) -> bool {
    let mut i = 0;
    while i < src.len() {
        if src[i] != '\\' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while end < src.len() && src[end].is_ascii_alphabetic() {
            end += 1;
        }
        let name: String = src[start..end].iter().collect();
        i = end.max(start + 1);
        if !matches!(
            name.as_str(),
            "frac" | "dfrac" | "tfrac" | "cfrac" | "binom"
        ) {
            continue;
        }
        let mut j = i;
        for arg in 0..2 {
            skip_space(src, &mut j);
            match src.get(j) {
                None => return false,
                Some('{') => match group_end(src, j) {
                    Some(end) => j = end,
                    None if arg == 0 => return false,
                    None => j = src.len(),
                },
                Some('\\') => {
                    j += 1;
                    while j < src.len() && src[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                }
                Some(_) => j += 1,
            }
        }
    }
    true
}

/// Command and environment names in a formula, without the backslash.
fn commands(latex: &str) -> impl Iterator<Item = String> + '_ {
    let mut rest = latex;
    std::iter::from_fn(move || loop {
        let at = rest.find('\\')?;
        let after = &rest[at + 1..];
        let len = after
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(after.len());
        if len == 0 {
            rest = after.get(1..).unwrap_or("");
            continue;
        }
        let name = &after[..len];
        rest = &after[len..];
        if name == "begin" || name == "end" {
            if let Some(env) = rest.trim_start().strip_prefix('{') {
                if let Some(close) = env.find('}') {
                    return Some(env[..close].to_string());
                }
            }
        }
        return Some(name.to_string());
    })
}

/// Index just past the `}` closing the group that opens at `start`.
fn group_end(src: &[char], start: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = start;
    while i < src.len() {
        match src[i] {
            '\\' => i += 1,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// A `<label` with no closing `>` inside `\text{…}`: mitex hands the text to
/// Typst as markup, where it opens a label that never closes.
fn unclosed_label_in_text(latex: &str) -> bool {
    let src: Vec<char> = latex.chars().collect();
    let mut i = 0;
    while i < src.len() {
        if src[i] != '\\' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while end < src.len() && src[end].is_ascii_alphabetic() {
            end += 1;
        }
        let name: String = src[start..end].iter().collect();
        i = end.max(start + 1);
        if name.starts_with("text") || name == "hbox" {
            skip_space(&src, &mut i);
            if has_unclosed_label(&read_group(&src, &mut i)) {
                return true;
            }
        }
    }
    false
}

fn has_unclosed_label(text: &str) -> bool {
    let is_label = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':');
    text.match_indices('<').any(|(at, _)| {
        let rest = &text[at + 1..];
        let len = rest.find(|c: char| !is_label(c)).unwrap_or(rest.len());
        len > 0 && !rest[len..].starts_with('>')
    })
}

/// Every command `math-core` marked as unknown.
fn unknown_commands(mathml: &str) -> Vec<String> {
    mathml
        .split(UNKNOWN)
        .skip(1)
        .filter_map(|rest| rest.split_once("<mtext>"))
        .filter_map(|(_, rest)| rest.split_once("</mtext>"))
        .map(|(name, _)| name.trim().trim_start_matches('\\').to_string())
        .collect()
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

    /// The PDF side of the parity suite. Each formula was rendered through
    /// the vendored mitex in a real Typst compile (0.15.1): `ok` compiled,
    /// `fails` aborted it. The engine has to give every `ok` formula to mitex
    /// unchanged — falling back on one would be a regression the PDF never
    /// had — and must catch every `fails` one before it reaches the compile.
    #[test]
    fn the_pdf_check_agrees_with_mitex() {
        let m = Math::new();
        for line in include_str!("math-corpus.tsv").lines() {
            let (formula, verdict) = line.rsplit_once('\t').unwrap();
            assert_eq!(
                m.pdf_accepts(formula),
                verdict == "ok",
                "{formula} ({verdict} in the PDF today)"
            );
        }
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
