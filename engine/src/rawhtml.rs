//! The raw HTML both renderers honour. Comments are dropped, a short list of
//! inline formatting tags renders as formatting (attributes dropped), and
//! every other tag stays visible text.

use comrak::nodes::{AstNode, NodeValue};

/// Inline tags with a formatting meaning in both outputs.
const INLINE_TAGS: &[&str] = &[
    "b", "strong", "i", "em", "u", "ins", "s", "del", "strike", "sub", "sup", "kbd", "mark",
    "small", "br",
];

#[derive(Debug, PartialEq)]
pub(crate) enum Raw<'a> {
    Comment,
    Open(&'static str),
    Close(&'static str),
    Other(&'a str),
}

/// Classify one raw inline HTML token.
pub(crate) fn classify(literal: &str) -> Raw<'_> {
    let t = literal.trim();
    if t.starts_with("<!--") && t.ends_with("-->") {
        return Raw::Comment;
    }
    let Some(inner) = t.strip_prefix('<').and_then(|r| r.strip_suffix('>')) else {
        return Raw::Other(literal);
    };
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let name_len = inner
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(inner.len());
    let (name, rest) = inner.split_at(name_len);
    let Some(tag) = INLINE_TAGS
        .iter()
        .find(|t| t.eq_ignore_ascii_case(name))
        .copied()
    else {
        return Raw::Other(literal);
    };
    // Attributes need a space before them; `/` closes a void element.
    let self_closing = rest.trim_end().ends_with('/');
    if !(rest.is_empty() || rest.starts_with(char::is_whitespace) || rest == "/") {
        return Raw::Other(literal);
    }
    match (closing, self_closing, tag) {
        (false, _, "br") => Raw::Open("br"),
        (false, false, _) => Raw::Open(tag),
        (true, false, _) if rest.trim().is_empty() && tag != "br" => Raw::Close(tag),
        _ => Raw::Other(literal),
    }
}

/// The sibling closing the tag opened at `kids[open]`, minding nesting.
pub(crate) fn closing(kids: &[&AstNode<'_>], open: usize, tag: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, kid) in kids.iter().enumerate().skip(open + 1) {
        if let NodeValue::HtmlInline(h) = &kid.data.borrow().value {
            match classify(h) {
                Raw::Open(t) if t == tag => depth += 1,
                Raw::Close(t) if t == tag => {
                    if depth == 0 {
                        return Some(i);
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
    }
    None
}

/// An HTML block with its comments removed: empty when it was only comments.
pub(crate) fn without_comments(literal: &str) -> String {
    let mut out = String::new();
    let mut rest = literal;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_recognised_by_name_alone() {
        assert_eq!(classify("<kbd>"), Raw::Open("kbd"));
        assert_eq!(classify("<B class=\"x\">"), Raw::Open("b"));
        assert_eq!(classify("</small>"), Raw::Close("small"));
        assert_eq!(classify("<br/>"), Raw::Open("br"));
        assert_eq!(classify("<br class=x />"), Raw::Open("br"));
        assert_eq!(classify("<!-- c -->"), Raw::Comment);
        for other in ["<span>", "< u>", "<u/>", "<break>", "</ u>", "<bfoo>"] {
            assert_eq!(classify(other), Raw::Other(other), "{other}");
        }
    }

    #[test]
    fn comments_leave_html_blocks() {
        assert_eq!(without_comments("<!-- a -->\n"), "\n");
        assert_eq!(without_comments("<!-- a --> x <!-- b"), " x ");
    }
}
