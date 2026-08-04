//! LaTeX -> MathML. The PDF path delegates math to mitex inside Typst; HTML
//! needs real markup, and MathML Core renders natively in every current
//! browser — no JS, no webfont, sharp at any zoom, and it inherits the
//! surrounding colour so both themes work for free.

use math_core::{LatexToMathML, MathCoreConfig, MathDisplay};

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

    /// Render one formula. On a parse error the LaTeX source is shown verbatim
    /// so the author can see what to fix, matching how Typst surfaces bad math.
    pub(crate) fn render(&self, display: bool, latex: &str) -> String {
        let mode = if display {
            MathDisplay::Block
        } else {
            MathDisplay::Inline
        };
        let class = if display {
            "md2pdf-math md2pdf-math-block"
        } else {
            "md2pdf-math"
        };
        match self.0.convert_with_local_state(latex.trim(), mode) {
            Ok(result) => format!("<span class=\"{class}\">{}</span>", result.mathml),
            Err(err) => format!(
                "<code class=\"md2pdf-math-error\" title=\"{}\">{}</code>",
                super::esc_attr(&err.to_string()),
                super::esc_text(latex.trim())
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
