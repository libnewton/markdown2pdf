//! A small syntax highlighter for fenced code blocks.
//!
//! Typst highlights `raw` blocks natively for the PDF; HTML has to do it here.
//! One generic lexer (comments, strings, numbers, words) driven by per-language
//! keyword tables covers the languages people actually paste into Markdown,
//! plus a tag-aware lexer for HTML/XML. Colours come from CSS variables, so
//! both themes stay legible. Unknown languages fall through unhighlighted.

use super::esc_text;

/// A lexical class; the value is the CSS class suffix (`.md2pdf-t-<x>`).
#[derive(Clone, Copy, PartialEq)]
enum Tok {
    Comment,
    Str,
    Num,
    Keyword,
    Type,
    Meta,
    Plain,
}

impl Tok {
    fn class(self) -> Option<&'static str> {
        match self {
            Tok::Comment => Some("c"),
            Tok::Str => Some("s"),
            Tok::Num => Some("n"),
            Tok::Keyword => Some("k"),
            Tok::Type => Some("t"),
            Tok::Meta => Some("m"),
            Tok::Plain => None,
        }
    }
}

struct Lang {
    line: &'static [&'static str],
    block: &'static [(&'static str, &'static str)],
    quotes: &'static [char],
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    /// Sigil starting a meta token that runs to the end of the word
    /// (`@decorator`, `#[attr]`, `$var`, `--flag`).
    meta: &'static [&'static str],
}

const C_LINE: &[&str] = &["//"];
const C_BLOCK: &[(&str, &str)] = &[("/*", "*/")];
const HASH_LINE: &[&str] = &["#"];
const QUOTES: &[char] = &['"', '\''];

/// Resolve a fence info string to a language table.
fn lang_for(info: &str) -> Option<&'static Lang> {
    // `rust,ignore` / `js title=x` — only the first word names the language.
    let name = info
        .split(|c: char| c.is_whitespace() || c == ',')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    Some(match name.as_str() {
        "rust" | "rs" => &RUST,
        "js" | "javascript" | "mjs" | "jsx" => &JS,
        "ts" | "typescript" | "tsx" => &TS,
        "python" | "py" => &PYTHON,
        "go" | "golang" => &GO,
        "c" | "h" => &C,
        "cpp" | "c++" | "cc" | "hpp" => &CPP,
        "java" | "kotlin" | "kt" => &JAVA,
        "cs" | "csharp" => &CSHARP,
        "json" | "jsonc" => &JSON,
        "yaml" | "yml" => &YAML,
        "toml" | "ini" => &TOML,
        "sh" | "bash" | "zsh" | "shell" | "console" => &SHELL,
        "sql" => &SQL,
        "css" | "scss" | "less" => &CSS,
        "typ" | "typst" => &TYPST,
        "diff" | "patch" => &DIFF,
        _ => return None,
    })
}

macro_rules! lang {
    ($name:ident, line: $line:expr, block: $block:expr, quotes: $q:expr,
     meta: $meta:expr, keywords: [$($kw:literal)*], types: [$($ty:literal)*]) => {
        static $name: Lang = Lang {
            line: $line,
            block: $block,
            quotes: $q,
            meta: $meta,
            keywords: &[$($kw),*],
            types: &[$($ty),*],
        };
    };
}

lang!(RUST, line: C_LINE, block: C_BLOCK, quotes: QUOTES, meta: &["#["],
    keywords: ["as" "async" "await" "break" "const" "continue" "crate" "dyn" "else" "enum" "extern"
        "false" "fn" "for" "if" "impl" "in" "let" "loop" "match" "mod" "move" "mut" "pub" "ref"
        "return" "self" "Self" "static" "struct" "super" "trait" "true" "type" "unsafe" "use"
        "where" "while"],
    types: ["bool" "char" "f32" "f64" "i8" "i16" "i32" "i64" "i128" "isize" "str" "u8" "u16" "u32"
        "u64" "u128" "usize" "String" "Vec" "Option" "Result" "Box" "Some" "None" "Ok" "Err"]);

lang!(JS, line: C_LINE, block: C_BLOCK, quotes: &['"', '\'', '`'], meta: &["@"],
    keywords: ["as" "async" "await" "break" "case" "catch" "class" "const" "continue" "debugger"
        "default" "delete" "do" "else" "export" "extends" "finally" "for" "from" "function" "get"
        "if" "import" "in" "instanceof" "let" "new" "of" "return" "set" "static" "super" "switch"
        "this" "throw" "try" "typeof" "var" "void" "while" "with" "yield"],
    types: ["Array" "Boolean" "Date" "Error" "false" "Infinity" "JSON" "Map" "Math" "NaN" "null"
        "Number" "Object" "Promise" "RegExp" "Set" "String" "Symbol" "true" "undefined"]);

lang!(TS, line: C_LINE, block: C_BLOCK, quotes: &['"', '\'', '`'], meta: &["@"],
    keywords: ["abstract" "as" "async" "await" "break" "case" "catch" "class" "const" "continue"
        "declare" "default" "delete" "do" "else" "enum" "export" "extends" "finally" "for" "from"
        "function" "get" "if" "implements" "import" "in" "instanceof" "interface" "keyof" "let"
        "new" "of" "private" "protected" "public" "readonly" "return" "satisfies" "set" "static"
        "super" "switch" "this" "throw" "try" "type" "typeof" "var" "void" "while" "yield"],
    types: ["any" "Array" "bigint" "boolean" "Date" "Error" "false" "JSON" "Map" "never" "null"
        "number" "object" "Object" "Promise" "Record" "Set" "string" "symbol" "true" "undefined"
        "unknown" "void"]);

lang!(PYTHON, line: HASH_LINE, block: &[("\"\"\"", "\"\"\""), ("'''", "'''")], quotes: QUOTES,
    meta: &["@"],
    keywords: ["and" "as" "assert" "async" "await" "break" "class" "continue" "def" "del" "elif"
        "else" "except" "finally" "for" "from" "global" "if" "import" "in" "is" "lambda" "nonlocal"
        "not" "or" "pass" "raise" "return" "try" "while" "with" "yield" "match" "case"],
    types: ["bool" "bytes" "dict" "False" "float" "frozenset" "int" "list" "None" "object" "self"
        "set" "str" "True" "tuple" "type"]);

lang!(GO, line: C_LINE, block: C_BLOCK, quotes: &['"', '\'', '`'], meta: &[],
    keywords: ["break" "case" "chan" "const" "continue" "default" "defer" "else" "fallthrough"
        "for" "func" "go" "goto" "if" "import" "interface" "map" "package" "range" "return"
        "select" "struct" "switch" "type" "var"],
    types: ["bool" "byte" "complex64" "complex128" "error" "false" "float32" "float64" "int"
        "int8" "int16" "int32" "int64" "nil" "rune" "string" "true" "uint" "uint8" "uint16"
        "uint32" "uint64" "uintptr"]);

lang!(C, line: C_LINE, block: C_BLOCK, quotes: QUOTES, meta: &["#"],
    keywords: ["auto" "break" "case" "const" "continue" "default" "do" "else" "enum" "extern"
        "for" "goto" "if" "inline" "register" "restrict" "return" "sizeof" "static" "struct"
        "switch" "typedef" "union" "volatile" "while"],
    types: ["bool" "char" "double" "float" "int" "long" "NULL" "short" "signed" "size_t"
        "unsigned" "void"]);

lang!(CPP, line: C_LINE, block: C_BLOCK, quotes: QUOTES, meta: &["#"],
    keywords: ["auto" "break" "case" "catch" "class" "const" "constexpr" "continue" "default"
        "delete" "do" "else" "enum" "explicit" "export" "extern" "for" "friend" "goto" "if"
        "inline" "namespace" "new" "noexcept" "operator" "override" "private" "protected" "public"
        "return" "sizeof" "static" "struct" "switch" "template" "this" "throw" "try" "typedef"
        "typename" "union" "using" "virtual" "volatile" "while"],
    types: ["bool" "char" "double" "false" "float" "int" "long" "nullptr" "short" "signed"
        "size_t" "string" "true" "unsigned" "vector" "void"]);

lang!(JAVA, line: C_LINE, block: C_BLOCK, quotes: QUOTES, meta: &["@"],
    keywords: ["abstract" "break" "case" "catch" "class" "companion" "const" "continue" "data"
        "default" "do" "else" "enum" "extends" "final" "finally" "for" "fun" "if" "implements"
        "import" "in" "instanceof" "interface" "internal" "is" "native" "new" "object" "open"
        "override" "package" "private" "protected" "public" "return" "static" "super" "switch"
        "synchronized" "this" "throw" "throws" "transient" "try" "val" "var" "void" "volatile"
        "when" "while"],
    types: ["boolean" "Boolean" "byte" "char" "double" "Double" "false" "float" "int" "Integer"
        "List" "long" "Long" "Map" "null" "Object" "short" "String" "true"]);

lang!(CSHARP, line: C_LINE, block: C_BLOCK, quotes: QUOTES, meta: &["#", "["],
    keywords: ["abstract" "as" "async" "await" "base" "break" "case" "catch" "class" "const"
        "continue" "default" "delegate" "do" "else" "enum" "event" "explicit" "extern" "finally"
        "fixed" "for" "foreach" "get" "goto" "if" "implicit" "in" "interface" "internal" "is"
        "lock" "namespace" "new" "operator" "out" "override" "params" "private" "protected"
        "public" "readonly" "record" "ref" "return" "sealed" "set" "sizeof" "static" "struct"
        "switch" "this" "throw" "try" "typeof" "using" "var" "virtual" "void" "while" "yield"],
    types: ["bool" "byte" "char" "decimal" "double" "false" "float" "int" "List" "long" "null"
        "object" "sbyte" "short" "string" "true" "uint" "ulong" "ushort"]);

lang!(JSON, line: &[], block: &[], quotes: &['"'], meta: &[],
    keywords: ["true" "false" "null"], types: []);

lang!(YAML, line: HASH_LINE, block: &[], quotes: QUOTES, meta: &[],
    keywords: ["false" "no" "null" "off" "on" "true" "yes" "~"], types: []);

lang!(TOML, line: HASH_LINE, block: &[], quotes: QUOTES, meta: &[],
    keywords: ["false" "true"], types: []);

lang!(SHELL, line: HASH_LINE, block: &[], quotes: &['"', '\'', '`'], meta: &["$", "--", "-"],
    keywords: ["case" "cd" "do" "done" "echo" "elif" "else" "esac" "exit" "export" "fi" "for"
        "function" "if" "in" "local" "read" "return" "set" "source" "then" "unset" "until"
        "while"],
    types: ["awk" "cat" "cp" "curl" "cut" "find" "git" "grep" "head" "ls" "make" "mkdir" "mv"
        "npm" "rm" "sed" "sort" "sudo" "tail" "tar" "wget"]);

lang!(SQL, line: &["--"], block: C_BLOCK, quotes: QUOTES, meta: &[],
    keywords: ["ALTER" "AND" "AS" "ASC" "BY" "CASE" "CREATE" "DELETE" "DESC" "DISTINCT" "DROP"
        "ELSE" "END" "EXISTS" "FROM" "FULL" "GROUP" "HAVING" "IN" "INDEX" "INNER" "INSERT" "INTO"
        "IS" "JOIN" "LEFT" "LIKE" "LIMIT" "NOT" "NULL" "OFFSET" "ON" "OR" "ORDER" "OUTER" "RIGHT"
        "SELECT" "SET" "TABLE" "THEN" "UNION" "UPDATE" "VALUES" "VIEW" "WHEN" "WHERE" "WITH"],
    types: ["BIGINT" "BOOLEAN" "CHAR" "DATE" "DECIMAL" "FLOAT" "INT" "INTEGER" "JSON" "NUMERIC"
        "SERIAL" "TEXT" "TIMESTAMP" "UUID" "VARCHAR"]);

lang!(CSS, line: &[], block: C_BLOCK, quotes: QUOTES, meta: &["@", "--"],
    keywords: ["and" "from" "important" "inherit" "initial" "not" "revert" "to" "unset" "var"],
    types: []);

lang!(TYPST, line: C_LINE, block: C_BLOCK, quotes: &['"'], meta: &["#"],
    keywords: ["and" "as" "auto" "break" "continue" "else" "false" "for" "if" "import" "in"
        "include" "let" "none" "not" "or" "return" "set" "show" "true" "while"],
    types: []);

lang!(DIFF, line: &[], block: &[], quotes: &[], meta: &[], keywords: [], types: []);

/// Highlight `code`, returning HTML. Falls back to plain escaped text when the
/// language is unknown.
pub(crate) fn highlight(info: &str, code: &str) -> String {
    let name = info.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    match name.as_str() {
        "html" | "xml" | "svg" | "vue" | "svelte" => return markup(code),
        "diff" | "patch" => return diff(code),
        _ => {}
    }
    match lang_for(info) {
        Some(lang) => generic(lang, code),
        None => esc_text(code),
    }
}

/// Emit one token, skipping the wrapper for unstyled text.
fn push(out: &mut String, tok: Tok, text: &str) {
    if text.is_empty() {
        return;
    }
    match tok.class() {
        Some(class) => {
            out.push_str("<span class=\"md2pdf-t-");
            out.push_str(class);
            out.push_str("\">");
            out.push_str(&esc_text(text));
            out.push_str("</span>");
        }
        None => out.push_str(&esc_text(text)),
    }
}

fn generic(lang: &Lang, code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let bytes = code.as_bytes();
    let mut i = 0;
    let mut plain_start = 0;

    // Flush the pending run of unstyled source, then emit `tok`.
    macro_rules! emit {
        ($end:expr, $tok:expr) => {{
            push(&mut out, Tok::Plain, &code[plain_start..i]);
            push(&mut out, $tok, &code[i..$end]);
            i = $end;
            plain_start = i;
        }};
    }

    while i < bytes.len() {
        let rest = &code[i..];

        if let Some(open) = lang.line.iter().find(|p| rest.starts_with(**p)) {
            // A shell `#` mid-word is a fragment identifier, not a comment.
            let standalone = *open != "#" || i == 0 || bytes[i - 1].is_ascii_whitespace();
            if standalone {
                let end = rest.find('\n').map_or(code.len(), |n| i + n);
                emit!(end, Tok::Comment);
                continue;
            }
        }
        if let Some((open, close)) = lang.block.iter().find(|(o, _)| rest.starts_with(*o)) {
            let from = i + open.len();
            let end = code[from..]
                .find(close)
                .map_or(code.len(), |n| from + n + close.len());
            emit!(end, Tok::Comment);
            continue;
        }
        let c = rest.chars().next().unwrap();
        if lang.quotes.contains(&c) {
            emit!(string_end(code, i, c), Tok::Str);
            continue;
        }
        if c.is_ascii_digit() && !prev_is_word(bytes, i) {
            let end = i + rest
                .find(|d: char| !(d.is_ascii_alphanumeric() || d == '.' || d == '_'))
                .unwrap_or(rest.len());
            emit!(end, Tok::Num);
            continue;
        }
        // Only at a word boundary, so `foo-bar` is not read as a `-bar` flag.
        let sigil = lang.meta.iter().find(|s| rest.starts_with(**s));
        if let Some(sigil) = sigil.filter(|_| !prev_is_word(bytes, i)) {
            let after = &rest[sigil.len()..];
            let len = after
                .find(|d: char| !(d.is_alphanumeric() || d == '_' || d == '-'))
                .unwrap_or(after.len());
            if len > 0 {
                emit!(i + sigil.len() + len, Tok::Meta);
                continue;
            }
        }
        if c.is_alphabetic() || c == '_' {
            let len = rest
                .find(|d: char| !(d.is_alphanumeric() || d == '_'))
                .unwrap_or(rest.len());
            let word = &rest[..len];
            let tok = if lang.keywords.contains(&word) {
                Tok::Keyword
            } else if lang.types.contains(&word) {
                Tok::Type
            } else {
                Tok::Plain
            };
            if tok != Tok::Plain {
                emit!(i + len, tok);
            } else {
                i += len;
            }
            continue;
        }
        i += c.len_utf8();
    }
    push(&mut out, Tok::Plain, &code[plain_start..]);
    out
}

/// End offset of a quoted string starting at `start`, honouring backslash
/// escapes and stopping at end of line for single-line quotes.
fn string_end(code: &str, start: usize, quote: char) -> usize {
    let mut chars = code[start + quote.len_utf8()..].char_indices();
    let base = start + quote.len_utf8();
    while let Some((offset, c)) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c == quote {
            return base + offset + quote.len_utf8();
        } else if c == '\n' && quote != '`' {
            return base + offset;
        }
    }
    code.len()
}

fn prev_is_word(bytes: &[u8], i: usize) -> bool {
    i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_')
}

/// Tag-aware lexer for HTML/XML: tag names as keywords, attribute names as
/// types, quoted values as strings.
fn markup(code: &str) -> String {
    let mut out = String::new();
    let mut rest = code;
    while let Some(open) = rest.find('<') {
        push(&mut out, Tok::Plain, &rest[..open]);
        rest = &rest[open..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            let end = comment.find("-->").map_or(rest.len(), |n| n + 4 + 3);
            push(&mut out, Tok::Comment, &rest[..end]);
            rest = &rest[end..];
            continue;
        }
        let Some(name_end) = tag_name_end(rest) else {
            // A bare `<` in prose, not a tag.
            push(&mut out, Tok::Plain, "<");
            rest = &rest[1..];
            continue;
        };
        let close = rest.find('>').map_or(rest.len(), |n| n + 1);
        push(&mut out, Tok::Keyword, &rest[..name_end]);
        out.push_str(&markup_attrs(&rest[name_end..close]));
        rest = &rest[close..];
    }
    push(&mut out, Tok::Plain, rest);
    out
}

/// End offset of `<tag` / `</tag`, or `None` when this is not a tag at all.
fn tag_name_end(tag: &str) -> Option<usize> {
    let lead = if tag.starts_with("</") { 2 } else { 1 };
    let rest = tag.get(lead..)?;
    let len = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == ':' || c == '!' || c == '?'))
        .unwrap_or(rest.len());
    (len > 0).then_some(lead + len)
}

fn markup_attrs(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while !rest.is_empty() {
        let c = rest.chars().next().unwrap();
        if c == '"' || c == '\'' {
            let end = string_end(rest, 0, c);
            push(&mut out, Tok::Str, &rest[..end]);
            rest = &rest[end..];
        } else if c.is_alphabetic() || c == '-' || c == ':' || c == '@' {
            let end = rest
                .find(|d: char| !(d.is_alphanumeric() || "-:@_.".contains(d)))
                .unwrap_or(rest.len());
            push(&mut out, Tok::Type, &rest[..end]);
            rest = &rest[end..];
        } else {
            let end = c.len_utf8();
            push(&mut out, Tok::Plain, &rest[..end]);
            rest = &rest[end..];
        }
    }
    out
}

/// Unified-diff colouring: whole lines, by their first character.
fn diff(code: &str) -> String {
    let mut out = String::new();
    for (n, line) in code.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let tok = match line.as_bytes().first() {
            Some(b'+') if !line.starts_with("+++") => Tok::Type,
            Some(b'-') if !line.starts_with("---") => Tok::Keyword,
            Some(b'@') => Tok::Meta,
            Some(b'+') | Some(b'-') | Some(b'd') | Some(b'i') => Tok::Comment,
            _ => Tok::Plain,
        };
        push(&mut out, tok, line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_languages_are_escaped_but_not_highlighted() {
        let out = highlight("brainfuck", "a < b");
        assert_eq!(out, "a &lt; b");
    }

    #[test]
    fn keywords_types_numbers_and_strings_get_classes() {
        let out = highlight("rust", "let x: u8 = 3; // hi\nlet s = \"a\";");
        assert!(out.contains("md2pdf-t-k\">let<"), "{out}");
        assert!(out.contains("md2pdf-t-t\">u8<"), "{out}");
        assert!(out.contains("md2pdf-t-n\">3<"), "{out}");
        assert!(out.contains("md2pdf-t-c\">// hi<"), "{out}");
        assert!(out.contains("md2pdf-t-s\">\"a\"<"), "{out}");
    }

    #[test]
    fn a_keyword_inside_an_identifier_is_not_highlighted() {
        let out = highlight("rust", "letter deferred");
        assert!(!out.contains("md2pdf-t-k"), "{out}");
    }

    #[test]
    fn strings_stop_at_the_closing_quote_and_honour_escapes() {
        let out = highlight("js", r#"const a = "x\"y"; const b = 1;"#);
        assert!(out.contains(r#"md2pdf-t-s">"x\"y"<"#), "{out}");
        assert!(out.contains("md2pdf-t-n\">1<"), "{out}");
    }

    #[test]
    fn an_unterminated_string_does_not_swallow_the_rest_of_the_block() {
        let out = highlight("python", "s = \"oops\nprint(1)");
        assert!(out.contains("print"), "{out}");
    }

    #[test]
    fn block_comments_are_spanned_whole() {
        let out = highlight("c", "/* a\nb */ int x;");
        assert!(out.contains("md2pdf-t-c\">/* a\nb */<"), "{out}");
        assert!(out.contains("md2pdf-t-t\">int<"), "{out}");
    }

    #[test]
    fn markup_colours_tags_attributes_and_values() {
        let out = highlight("html", "<a href=\"x\">t</a>");
        assert!(out.contains("md2pdf-t-k\">&lt;a<"), "{out}");
        assert!(out.contains("md2pdf-t-t\">href<"), "{out}");
        assert!(out.contains("md2pdf-t-s\">\"x\"<"), "{out}");
        assert!(out.contains("&gt;t<span"), "{out}");
    }

    #[test]
    fn diff_lines_are_coloured_by_their_marker() {
        let out = highlight("diff", "@@ -1 +1 @@\n-old\n+new");
        assert!(out.contains("md2pdf-t-m\">@@ -1 +1 @@<"), "{out}");
        assert!(out.contains("md2pdf-t-k\">-old<"), "{out}");
        assert!(out.contains("md2pdf-t-t\">+new<"), "{out}");
    }

    #[test]
    fn info_string_suffixes_still_resolve_the_language() {
        assert!(highlight("rust,ignore", "let x = 1;").contains("md2pdf-t-k"));
        assert!(highlight("ts title=a.ts", "const x = 1").contains("md2pdf-t-k"));
    }

    #[test]
    fn highlighting_never_loses_source_text() {
        for (lang, code) in [
            ("rust", "fn main() { println!(\"{}\", 1 + 2); }"),
            ("python", "@dec\ndef f(a=1): return a # c"),
            ("shell", "cd /tmp && grep -r 'x' . # note"),
            ("yaml", "a: 1\nb: \"two\" # c"),
            ("sql", "SELECT * FROM t WHERE a = 1; -- c"),
            ("css", "a { color: #fff; /* c */ }"),
            ("html", "<p class='a'>x &amp; y</p>"),
            ("json", "{\"a\": [1, true, null]}"),
        ] {
            let out = highlight(lang, code);
            let stripped = strip_tags(&out);
            assert_eq!(stripped, esc_text(code), "lang {lang}");
        }
    }

    fn strip_tags(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;
        while let Some(i) = rest.find('<') {
            out.push_str(&rest[..i]);
            rest = &rest[rest[i..].find('>').map(|j| i + j + 1).unwrap_or(rest.len())..];
        }
        out.push_str(rest);
        out
    }
}
