//! A syntax highlighter for fenced code blocks.
//!
//! Typst highlights `raw` blocks natively for the PDF; HTML has to do it here.
//! One generic lexer covers the ~40 languages people paste into Markdown,
//! driven by a per-language table that describes its comment forms, string
//! forms, keywords, types, builtins and sigils rather than just a word list.
//! Three shapes are different enough to want their own pass: markup
//! (HTML/XML/SVG), stylesheets, and unified diffs. A language we do not know
//! still gets comments, strings and numbers.
//!
//! The lexer keeps exactly one bit of context — whether the last thing read
//! could end an expression — which is what tells a regex literal from a
//! division. Everything else is local.
//!
//! Colours come from CSS variables and are held to WCAG AA in both themes by
//! `tokens.rs`'s test. `highlighting_never_loses_source_text` is the load-bearing
//! one: every language, over hostile input, must reproduce the source exactly
//! and escaped.

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
    Func,
    Operator,
    Property,
    Variable,
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
            Tok::Func => Some("f"),
            Tok::Operator => Some("o"),
            Tok::Property => Some("p"),
            Tok::Variable => Some("v"),
            Tok::Plain => None,
        }
    }
}

/// A string delimiter pair. Spelling these out is what lets Python's `"""`,
/// a JS template literal and a Rust raw string each end where they should
/// instead of at the next matching quote character.
#[derive(Clone, Copy)]
struct StrForm {
    open: &'static str,
    close: &'static str,
    /// Whether the literal may cross a line break.
    multiline: bool,
    /// Whether `\` escapes the following character.
    escapes: bool,
}

const fn s(open: &'static str) -> StrForm {
    StrForm { open, close: open, multiline: false, escapes: true }
}
const fn multi(open: &'static str, close: &'static str) -> StrForm {
    StrForm { open, close, multiline: true, escapes: true }
}
/// A raw literal: no escape processing, so `r"C:\path\"` ends at the quote.
const fn raw(open: &'static str, close: &'static str) -> StrForm {
    StrForm { open, close, multiline: true, escapes: false }
}

struct Lang {
    line: &'static [&'static str],
    block: &'static [(&'static str, &'static str)],
    /// Tried in order before `quotes`, so a longer opener wins over a shorter.
    strings: &'static [StrForm],
    quotes: &'static [char],
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    /// Names that are neither syntax nor types — `print`, `console`, `nil`.
    builtins: &'static [&'static str],
    /// Sigil starting a meta token that runs to the end of the word
    /// (`@decorator`, `#[attr]`, `$var`, `--flag`).
    meta: &'static [&'static str],
    /// `/` can open a regex literal here, so it needs the division check.
    regex: bool,
    /// `.member` reads as a property rather than as plain text.
    properties: bool,
    /// `key:` is a mapping key — YAML at line start, JSON as a quoted string.
    /// Without it a config file is one flat colour, which is most of what
    /// makes the generic lexer look thin on the formats people paste most.
    keys: bool,
}

/// Everything a language does not say otherwise. Declarations name only the
/// fields that differ, which is what keeps forty of them readable.
const DEFAULT: Lang = Lang {
    line: &[],
    block: &[],
    strings: &[],
    quotes: &[],
    keywords: &[],
    types: &[],
    builtins: &[],
    meta: &[],
    regex: false,
    properties: true,
    keys: false,
};

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
        "php" => &PHP,
        "ruby" | "rb" => &RUBY,
        "swift" => &SWIFT,
        "lua" => &LUA,
        "r" => &R,
        "dart" => &DART,
        "scala" => &SCALA,
        "perl" | "pl" => &PERL,
        "powershell" | "pwsh" | "ps1" => &POWERSHELL,
        "dockerfile" | "docker" => &DOCKERFILE,
        "makefile" | "make" | "mk" => &MAKE,
        "graphql" | "gql" => &GRAPHQL,
        "protobuf" | "proto" => &PROTOBUF,
        "hcl" | "terraform" | "tf" => &HCL,
        "nix" => &NIX,
        "zig" => &ZIG,
        "elixir" | "ex" | "exs" => &ELIXIR,
        "haskell" | "hs" => &HASKELL,
        "latex" | "tex" => &LATEX,
        "julia" | "jl" => &JULIA,
        _ => return None,
    })
}

macro_rules! lang {
    ($name:ident, line: $line:expr, block: $block:expr, quotes: $q:expr,
     meta: $meta:expr, keywords: [$($kw:literal)*], types: [$($ty:literal)*]
     $(, $field:ident: $value:expr)* $(,)?) => {
        static $name: Lang = Lang {
            line: $line,
            block: $block,
            quotes: $q,
            meta: $meta,
            keywords: &[$($kw),*],
            types: &[$($ty),*],
            $($field: $value,)*
            ..DEFAULT
        };
    };
}

lang!(RUST, line: C_LINE, block: C_BLOCK, quotes: QUOTES, meta: &["#[", "#!["],
    keywords: ["as" "async" "await" "break" "const" "continue" "crate" "dyn" "else" "enum" "extern"
        "false" "fn" "for" "if" "impl" "in" "let" "loop" "match" "mod" "move" "mut" "pub" "ref"
        "return" "self" "Self" "static" "struct" "super" "trait" "true" "type" "unsafe" "use"
        "where" "while"],
    types: ["bool" "char" "f32" "f64" "i8" "i16" "i32" "i64" "i128" "isize" "str" "u8" "u16" "u32"
        "u64" "u128" "usize" "String" "Vec" "Option" "Result" "Box" "Some" "None" "Ok" "Err"],
    strings: &[raw("r#\"", "\"#"), raw("br#\"", "\"#"), raw("r\"", "\"")],
    builtins: &["assert", "assert_eq", "format", "matches", "panic", "print", "println", "todo", "unimplemented", "unreachable", "vec", "write", "writeln"],
);

lang!(JS, line: C_LINE, block: C_BLOCK, quotes: &['"', '\''], meta: &["@"],
    keywords: ["as" "async" "await" "break" "case" "catch" "class" "const" "continue" "debugger"
        "default" "delete" "do" "else" "export" "extends" "finally" "for" "from" "function" "get"
        "if" "import" "in" "instanceof" "let" "new" "of" "return" "set" "static" "super" "switch"
        "this" "throw" "try" "typeof" "var" "void" "while" "with" "yield"],
    types: ["Array" "Boolean" "Date" "Error" "false" "Infinity" "JSON" "Map" "Math" "NaN" "null"
        "Number" "Object" "Promise" "RegExp" "Set" "String" "Symbol" "true" "undefined"],
    strings: &[multi("`", "`")], regex: true,
    builtins: &["console", "document", "fetch", "globalThis", "process", "require", "window"],
);

lang!(TS, line: C_LINE, block: C_BLOCK, quotes: &['"', '\''], meta: &["@"],
    keywords: ["abstract" "as" "async" "await" "break" "case" "catch" "class" "const" "continue"
        "declare" "default" "delete" "do" "else" "enum" "export" "extends" "finally" "for" "from"
        "function" "get" "if" "implements" "import" "in" "instanceof" "interface" "keyof" "let"
        "new" "of" "private" "protected" "public" "readonly" "return" "satisfies" "set" "static"
        "super" "switch" "this" "throw" "try" "type" "typeof" "var" "void" "while" "yield"],
    types: ["any" "Array" "bigint" "boolean" "Date" "Error" "false" "JSON" "Map" "never" "null"
        "number" "object" "Object" "Promise" "Record" "Set" "string" "symbol" "true" "undefined"
        "unknown" "void"],
    strings: &[multi("`", "`")], regex: true,
    builtins: &["console", "document", "fetch", "globalThis", "process", "require", "window"],
);

lang!(PYTHON, line: HASH_LINE, block: &[], quotes: QUOTES,
    meta: &["@"],
    keywords: ["and" "as" "assert" "async" "await" "break" "class" "continue" "def" "del" "elif"
        "else" "except" "finally" "for" "from" "global" "if" "import" "in" "is" "lambda" "nonlocal"
        "not" "or" "pass" "raise" "return" "try" "while" "with" "yield" "match" "case"],
    types: ["bool" "bytes" "dict" "False" "float" "frozenset" "int" "list" "None" "object" "self"
        "set" "str" "True" "tuple" "type"],
    // Triple quotes first, so a docstring is one string and not an empty one
    // followed by an unterminated one. The prefixed forms cover f/r/b strings.
    strings: &[
        multi("\"\"\"", "\"\"\""), multi("'''", "'''"),
        raw("r\"", "\""), raw("r'", "'"), raw("rb\"", "\""), raw("br\"", "\""),
        s("f\""), s("f'"), s("b\""), s("b'"), s("u\""),
    ],
    builtins: &["abs", "all", "any", "enumerate", "filter", "format", "input", "isinstance",
        "len", "map", "max", "min", "open", "print", "range", "repr", "reversed", "round",
        "sorted", "sum", "super", "zip"],
);

lang!(GO, line: C_LINE, block: C_BLOCK, quotes: &['"', '\''], meta: &[],
    keywords: ["break" "case" "chan" "const" "continue" "default" "defer" "else" "fallthrough"
        "for" "func" "go" "goto" "if" "import" "interface" "map" "package" "range" "return"
        "select" "struct" "switch" "type" "var"],
    types: ["bool" "byte" "complex64" "complex128" "error" "false" "float32" "float64" "int"
        "int8" "int16" "int32" "int64" "nil" "rune" "string" "true" "uint" "uint8" "uint16"
        "uint32" "uint64" "uintptr"],
    strings: &[raw("`", "`")],
    builtins: &["append", "cap", "close", "copy", "delete", "len", "make", "new", "panic", "print", "println", "recover"],
);

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
    keywords: ["true" "false" "null"], types: [], keys: true, properties: false);

lang!(YAML, line: HASH_LINE, block: &[], quotes: QUOTES, meta: &["&", "*", "!!"],
    keywords: ["false" "no" "null" "off" "on" "true" "yes" "~"], types: [],
    keys: true, properties: false);

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

lang!(PHP, line: &["//", "#"], block: C_BLOCK, quotes: QUOTES, meta: &["$"],
    keywords: ["abstract" "as" "break" "callable" "case" "catch" "class" "clone" "const" "continue"
        "declare" "default" "do" "echo" "else" "elseif" "enum" "extends" "final" "finally" "fn"
        "for" "foreach" "function" "global" "if" "implements" "include" "instanceof" "interface"
        "match" "namespace" "new" "print" "private" "protected" "public" "readonly" "require"
        "return" "static" "switch" "throw" "trait" "try" "use" "var" "while" "yield"],
    types: ["array" "bool" "false" "float" "int" "iterable" "mixed" "null" "object" "self" "string"
        "true" "void"]);

lang!(RUBY, line: HASH_LINE, block: &[("=begin", "=end")], quotes: QUOTES, meta: &["@", "$", ":"],
    keywords: ["alias" "begin" "break" "case" "class" "def" "defined?" "do" "else" "elsif" "end"
        "ensure" "for" "if" "in" "module" "next" "not" "or" "and" "redo" "rescue" "retry" "return"
        "self" "super" "then" "unless" "until" "when" "while" "yield" "require" "require_relative"
        "attr_accessor" "attr_reader" "attr_writer"],
    types: ["Array" "false" "Float" "Hash" "Integer" "nil" "Proc" "Range" "String" "Symbol" "true"]);

lang!(SWIFT, line: C_LINE, block: C_BLOCK, quotes: &['"'], meta: &["@"],
    keywords: ["actor" "as" "associatedtype" "async" "await" "break" "case" "catch" "class"
        "continue" "default" "defer" "deinit" "do" "else" "enum" "extension" "fallthrough" "for"
        "func" "guard" "if" "import" "in" "init" "inout" "internal" "is" "let" "nonisolated" "open"
        "operator" "private" "protocol" "public" "repeat" "return" "self" "some" "struct"
        "subscript" "switch" "throw" "throws" "try" "typealias" "var" "where" "while"],
    types: ["Any" "Array" "Bool" "Character" "Dictionary" "Double" "false" "Float" "Int" "nil"
        "Optional" "Result" "Set" "String" "true" "UInt" "Void"]);

lang!(LUA, line: &["--"], block: &[("--[[", "]]")], quotes: QUOTES, meta: &[],
    keywords: ["and" "break" "do" "else" "elseif" "end" "for" "function" "goto" "if" "in" "local"
        "not" "or" "repeat" "return" "then" "until" "while"],
    types: ["false" "nil" "self" "true"]);

lang!(R, line: HASH_LINE, block: &[], quotes: QUOTES, meta: &[],
    keywords: ["break" "else" "for" "function" "if" "in" "next" "repeat" "return" "while"],
    types: ["c" "character" "data.frame" "FALSE" "Inf" "integer" "list" "logical" "NA" "NaN"
        "NULL" "numeric" "TRUE" "vector"]);

lang!(DART, line: C_LINE, block: C_BLOCK, quotes: QUOTES, meta: &["@"],
    keywords: ["abstract" "as" "assert" "async" "await" "break" "case" "catch" "class" "const"
        "continue" "covariant" "default" "deferred" "do" "else" "enum" "export" "extends"
        "extension" "external" "factory" "final" "finally" "for" "get" "if" "implements" "import"
        "in" "is" "late" "library" "mixin" "new" "on" "operator" "part" "required" "rethrow"
        "return" "sealed" "set" "show" "static" "super" "switch" "sync" "this" "throw" "try"
        "typedef" "var" "while" "with" "yield"],
    types: ["bool" "double" "dynamic" "false" "Future" "int" "List" "Map" "null" "num" "Object"
        "Set" "Stream" "String" "true" "void"]);

lang!(SCALA, line: C_LINE, block: C_BLOCK, quotes: &['"'], meta: &["@"],
    keywords: ["abstract" "case" "catch" "class" "def" "do" "else" "enum" "extends" "final"
        "finally" "for" "forSome" "given" "if" "implicit" "import" "lazy" "match" "new" "object"
        "override" "package" "private" "protected" "return" "sealed" "super" "then" "this" "throw"
        "trait" "try" "type" "using" "val" "var" "while" "with" "yield"],
    types: ["Any" "AnyRef" "Boolean" "Byte" "Char" "Double" "false" "Float" "Int" "List" "Long"
        "Map" "Nothing" "null" "Option" "Seq" "Set" "Short" "String" "true" "Unit"]);

lang!(PERL, line: HASH_LINE, block: &[], quotes: QUOTES, meta: &["$", "@", "%"],
    keywords: ["do" "else" "elsif" "eval" "for" "foreach" "if" "last" "local" "my" "next" "our"
        "package" "redo" "require" "return" "sub" "unless" "until" "use" "wantarray" "while"],
    types: ["defined" "delete" "exists" "keys" "ref" "scalar" "undef" "values"]);

lang!(POWERSHELL, line: HASH_LINE, block: &[("<#", "#>")], quotes: QUOTES, meta: &["$", "-"],
    keywords: ["begin" "break" "catch" "class" "continue" "data" "do" "dynamicparam" "else"
        "elseif" "end" "enum" "exit" "filter" "finally" "for" "foreach" "function" "if" "in"
        "param" "process" "return" "switch" "throw" "trap" "try" "until" "using" "while"],
    types: ["$false" "$null" "$true" "bool" "hashtable" "int" "pscustomobject" "string"]);

lang!(DOCKERFILE, line: HASH_LINE, block: &[], quotes: QUOTES, meta: &[],
    keywords: ["ADD" "ARG" "CMD" "COPY" "ENTRYPOINT" "ENV" "EXPOSE" "FROM" "HEALTHCHECK" "LABEL"
        "ONBUILD" "RUN" "SHELL" "STOPSIGNAL" "USER" "VOLUME" "WORKDIR"],
    types: ["AS"]);

lang!(MAKE, line: HASH_LINE, block: &[], quotes: QUOTES, meta: &["$"],
    keywords: ["define" "else" "endef" "endif" "export" "ifdef" "ifeq" "ifndef" "ifneq" "include"
        "override" "unexport" "vpath"],
    types: [".PHONY" ".DEFAULT" ".SUFFIXES"]);

lang!(GRAPHQL, line: HASH_LINE, block: &[], quotes: &['"'], meta: &["@", "$"],
    keywords: ["directive" "enum" "extend" "fragment" "implements" "input" "interface" "mutation"
        "on" "query" "scalar" "schema" "subscription" "type" "union"],
    types: ["Boolean" "false" "Float" "ID" "Int" "null" "String" "true"]);

lang!(PROTOBUF, line: C_LINE, block: C_BLOCK, quotes: QUOTES, meta: &[],
    keywords: ["enum" "extend" "import" "message" "oneof" "option" "package" "public" "repeated"
        "reserved" "returns" "rpc" "service" "stream" "syntax"],
    types: ["bool" "bytes" "double" "false" "fixed32" "fixed64" "float" "int32" "int64" "map"
        "sfixed32" "sfixed64" "sint32" "sint64" "string" "true" "uint32" "uint64"]);

lang!(HCL, line: &["#", "//"], block: C_BLOCK, quotes: &['"'], meta: &["$"],
    keywords: ["data" "for" "for_each" "if" "in" "locals" "module" "output" "provider" "resource"
        "terraform" "variable"],
    types: ["bool" "false" "list" "map" "null" "number" "object" "set" "string" "true" "tuple"]);

lang!(NIX, line: HASH_LINE, block: C_BLOCK, quotes: &['"'], meta: &["$"],
    keywords: ["assert" "else" "if" "in" "inherit" "let" "or" "rec" "then" "with"],
    types: ["builtins" "false" "null" "true"]);

lang!(ZIG, line: C_LINE, block: &[], quotes: &['"', '\''], meta: &["@"],
    keywords: ["align" "and" "asm" "break" "catch" "comptime" "const" "continue" "defer" "else"
        "enum" "errdefer" "error" "export" "extern" "fn" "for" "if" "inline" "or" "orelse" "pub"
        "return" "struct" "switch" "test" "try" "union" "unreachable" "var" "while"],
    types: ["anytype" "bool" "f32" "f64" "false" "i8" "i16" "i32" "i64" "isize" "null" "true"
        "type" "u8" "u16" "u32" "u64" "undefined" "usize" "void"]);

lang!(ELIXIR, line: HASH_LINE, block: &[], quotes: &['"', '\''], meta: &["@", ":"],
    keywords: ["after" "case" "catch" "cond" "def" "defmacro" "defmodule" "defp" "defprotocol"
        "defstruct" "do" "else" "end" "fn" "for" "if" "import" "in" "raise" "receive" "require"
        "rescue" "try" "unless" "use" "when" "with"],
    types: ["false" "nil" "true"]);

lang!(HASKELL, line: &["--"], block: &[("{-", "-}")], quotes: QUOTES, meta: &[],
    keywords: ["case" "class" "data" "deriving" "do" "else" "foreign" "if" "import" "in" "infix"
        "infixl" "infixr" "instance" "let" "module" "newtype" "of" "then" "type" "where"],
    types: ["Bool" "Char" "Double" "Either" "False" "Float" "Int" "Integer" "IO" "Maybe" "Nothing"
        "Just" "String" "True"]);

lang!(LATEX, line: &["%"], block: &[], quotes: &[], meta: &["\\"],
    keywords: [], types: []);

lang!(JULIA, line: HASH_LINE, block: &[("#=", "=#")], quotes: QUOTES, meta: &["@"],
    keywords: ["abstract" "baremodule" "begin" "break" "catch" "const" "continue" "do" "else"
        "elseif" "end" "export" "finally" "for" "function" "global" "if" "import" "let" "local"
        "macro" "module" "mutable" "primitive" "quote" "return" "struct" "try" "using" "while"],
    types: ["Array" "Bool" "Dict" "false" "Float64" "Int" "Int64" "missing" "nothing" "String"
        "true" "Vector"]);

/// Highlight `code`, returning HTML. Falls back to plain escaped text when the
/// language is unknown.
pub(crate) fn highlight(info: &str, code: &str) -> String {
    let name = info.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    match name.as_str() {
        "html" | "xml" | "svg" | "vue" | "svelte" => return markup(code),
        "diff" | "patch" => return diff(code),
        "css" | "scss" | "less" => return stylesheet(code),
        _ => {}
    }
    match lang_for(info) {
        Some(lang) => generic(lang, code),
        // A fence naming a language we do not know still gets the shapes
        // every language shares. Rendering it flat was the single most
        // visible gap: one unlisted name and the whole block went grey.
        None if !name.is_empty() => generic(&FALLBACK, code),
        None => esc_text(code),
    }
}

/// Comments, strings, numbers and operators — true nearly everywhere, and
/// nothing that would be actively wrong if it is not.
static FALLBACK: Lang = Lang {
    line: &["//", "#", "--", ";"],
    block: C_BLOCK,
    quotes: QUOTES,
    ..DEFAULT
};

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
    // Whether the thing just read could end an expression. It is the one bit
    // of context the lexer keeps, and it is what tells `/` apart.
    let mut last_was_value = false;

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

        // Multi-character openers first: `"""` has to beat `"`, and `r#"` has
        // to beat both.
        if !prev_is_word(bytes, i) {
            if let Some(form) = lang.strings.iter().find(|f| rest.starts_with(f.open)) {
                emit!(form_end(code, i, form), Tok::Str);
                last_was_value = true;
                continue;
            }
        }
        if lang.quotes.contains(&c) {
            let end = string_end(code, i, c);
            // `"key": value` — a quoted string with a colon after it is a
            // mapping key, not a value.
            let tok = if lang.keys && followed_by_colon(&code[end..]) { Tok::Property } else { Tok::Str };
            emit!(end, tok);
            last_was_value = true;
            continue;
        }
        // `/` is division after a value and a regex literal otherwise. Without
        // this, `s.replace(/\/\//g, '')` turned the rest of the line into a
        // comment.
        if lang.regex && c == '/' && !last_was_value {
            if let Some(end) = regex_end(code, i) {
                emit!(end, Tok::Str);
                last_was_value = true;
                continue;
            }
        }
        if c.is_ascii_digit() && !prev_is_word(bytes, i) {
            emit!(number_end(rest, i), Tok::Num);
            last_was_value = true;
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
            let after_dot = lang.properties && i > 0 && bytes[i - 1] == b'.';
            // A bare word opening its line and followed by `:` is a YAML key.
            // Anchored to the line so a `https://` inside a value is not one.
            let tok = if lang.keys && starts_line(code, i) && followed_by_colon(&rest[len..]) {
                Tok::Property
            } else if lang.keywords.contains(&word) {
                Tok::Keyword
            } else if lang.types.contains(&word) {
                Tok::Type
            } else if lang.builtins.contains(&word) {
                Tok::Type
            } else if calls(&rest[len..]) {
                // A name with a `(` after it is being called or defined —
                // the most recognisable colour in any editor theme.
                Tok::Func
            } else if after_dot {
                Tok::Property
            } else if is_constant(word) {
                Tok::Variable
            } else {
                Tok::Plain
            };
            last_was_value = true;
            if tok != Tok::Plain {
                emit!(i + len, tok);
            } else {
                i += len;
            }
            continue;
        }
        // Operators as one run, so `!==` is a single span rather than three.
        if OPERATORS.contains(c) {
            let len = rest.find(|d: char| !OPERATORS.contains(d)).unwrap_or(rest.len());
            emit!(i + len, Tok::Operator);
            last_was_value = false;
            continue;
        }
        // A value can also end in a bracket: `xs[0] / 2` is division.
        if matches!(c, ')' | ']' | '}') {
            last_was_value = true;
        } else if !c.is_whitespace() {
            last_was_value = false;
        }
        i += c.len_utf8();
    }
    push(&mut out, Tok::Plain, &code[plain_start..]);
    out
}

/// Operator characters. Brackets, commas and semicolons stay plain: colouring
/// them adds noise without telling the reader anything.
const OPERATORS: &str = "+-*/%=<>!&|^~?";

fn followed_by_colon(after: &str) -> bool {
    let rest = after.trim_start_matches([' ', '\t']);
    rest.starts_with(':') && !rest.starts_with("::")
}

/// Whether only indentation and an optional `- ` list marker precede `i` on
/// its line.
fn starts_line(code: &str, i: usize) -> bool {
    let before = &code[..i];
    let line = before.rfind('\n').map_or(before, |n| &before[n + 1..]);
    matches!(line.trim(), "" | "-")
}

/// Whether a name is immediately applied — `foo(`, and `foo (` for the
/// languages that space it out.
fn calls(after: &str) -> bool {
    after.trim_start_matches([' ', '\t']).starts_with('(') && !after.starts_with('\n')
}

/// SCREAMING_SNAKE_CASE. The underscore is required on purpose: plain
/// all-caps is a table name in SQL and a type in half a dozen other
/// languages, and colouring those as constants is worse than missing `PI`.
fn is_constant(word: &str) -> bool {
    word.contains('_')
        && word.chars().any(|c| c.is_ascii_uppercase())
        && word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// End offset of a number, including base prefixes, digit separators,
/// exponents and type suffixes: `0xFF`, `1_000`, `1e-9`, `1.5f32`.
fn number_end(rest: &str, i: usize) -> usize {
    let bytes = rest.as_bytes();
    let mut n = 1;
    if bytes[0] == b'0' && rest.len() > 1 && matches!(bytes[1] | 32, b'x' | b'b' | b'o') {
        n = 2;
    }
    while n < rest.len() {
        let c = bytes[n];
        let exponent = matches!(c, b'+' | b'-')
            && n > 0
            && matches!(bytes[n - 1] | 32, b'e')
            && !rest[..n].starts_with("0x");
        if c.is_ascii_alphanumeric() || c == b'_' || exponent {
            n += 1;
        } else if c == b'.' && n + 1 < rest.len() && bytes[n + 1].is_ascii_digit() {
            n += 1;
        } else {
            break;
        }
    }
    i + n
}

/// End offset of a `/…/flags` regex literal, or `None` when the line ends
/// first — in which case it was division after all.
fn regex_end(code: &str, start: usize) -> Option<usize> {
    let mut chars = code[start + 1..].char_indices();
    let mut in_class = false;
    while let Some((offset, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '\n' => return None,
            '[' => in_class = true,
            ']' => in_class = false,
            '/' if !in_class => {
                let after = start + 1 + offset + 1;
                let flags = code[after..]
                    .find(|d: char| !d.is_ascii_alphabetic())
                    .unwrap_or(code.len() - after);
                return Some(after + flags);
            }
            _ => {}
        }
    }
    None
}

/// End offset of a literal opened by `form`.
fn form_end(code: &str, start: usize, form: &StrForm) -> usize {
    let from = start + form.open.len();
    let mut i = from;
    while i < code.len() {
        let rest = &code[i..];
        if form.escapes && rest.starts_with('\\') {
            i += 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
            continue;
        }
        if rest.starts_with(form.close) {
            return i + form.close.len();
        }
        let c = rest.chars().next().unwrap();
        if c == '\n' && !form.multiline {
            return i;
        }
        i += c.len_utf8();
    }
    code.len()
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

/// Stylesheets: selectors outside the braces, properties inside them.
///
/// The generic lexer has no way to tell those apart — it saw one long run of
/// bare words and left almost all of a stylesheet unstyled, which is why CSS
/// gets its own pass rather than another keyword table.
fn stylesheet(code: &str) -> String {
    let mut out = String::new();
    let mut rest = code;
    let mut in_block = false;

    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("/*") {
            let end = after.find("*/").map_or(rest.len(), |n| n + 2 + 2);
            push(&mut out, Tok::Comment, &rest[..end]);
            rest = &rest[end..];
            continue;
        }
        let c = rest.chars().next().unwrap();
        match c {
            '"' | '\'' => {
                let end = string_end(rest, 0, c);
                push(&mut out, Tok::Str, &rest[..end]);
                rest = &rest[end..];
            }
            '{' => {
                in_block = true;
                push(&mut out, Tok::Plain, "{");
                rest = &rest[1..];
            }
            '}' => {
                in_block = false;
                push(&mut out, Tok::Plain, "}");
                rest = &rest[1..];
            }
            // `@media`, `@import`, and the `!important` flag.
            '@' | '!' => {
                let end = word_end(&rest[1..]) + 1;
                push(&mut out, Tok::Meta, &rest[..end]);
                rest = &rest[end..];
            }
            // A selector fragment: `.class`, `#id`, `:hover`, `::before`.
            '.' | '#' | ':' if !in_block || c != ':' => {
                let lead = if rest.starts_with("::") { 2 } else { 1 };
                let end = word_end(&rest[lead..]) + lead;
                push(&mut out, if end > lead { Tok::Type } else { Tok::Plain }, &rest[..end]);
                rest = &rest[end..];
            }
            '0'..='9' => {
                // Keep the unit attached: `1.5rem` is one number, not two.
                let end = word_end_with(&rest[1..], |d| d.is_alphanumeric() || d == '.' || d == '%')
                    + 1;
                push(&mut out, Tok::Num, &rest[..end]);
                rest = &rest[end..];
            }
            _ if c.is_alphabetic() || c == '-' || c == '_' => {
                let end = word_end_with(rest, |d| d.is_alphanumeric() || d == '-' || d == '_');
                let name = &rest[..end];
                let tok = if in_block && followed_by_colon(&rest[end..]) {
                    Tok::Property
                } else if in_block {
                    Tok::Plain
                } else {
                    // An element selector.
                    Tok::Type
                };
                push(&mut out, tok, name);
                rest = &rest[end..];
            }
            _ => {
                let end = c.len_utf8();
                push(&mut out, Tok::Plain, &rest[..end]);
                rest = &rest[end..];
            }
        }
    }
    out
}

fn word_end(s: &str) -> usize {
    word_end_with(s, |c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn word_end_with(s: &str, ok: impl Fn(char) -> bool) -> usize {
    s.find(|c: char| !ok(c)).unwrap_or(s.len())
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

    /// An unlisted language still gets the shapes every language shares,
    /// rather than the flat grey block it used to be.
    #[test]
    fn an_unknown_language_gets_the_common_shapes() {
        let out = highlight("brainfuck", "x = \"s\" // c\n1");
        assert!(out.contains("md2pdf-t-s\">\"s\"<"), "{out}");
        assert!(out.contains("md2pdf-t-c\">// c<"), "{out}");
        assert!(out.contains("md2pdf-t-n\">1<"), "{out}");
        // Escaping is unchanged; the character is simply classed now.
        assert!(highlight("brainfuck", "a < b").contains("&lt;"));
    }

    /// A fence with no language at all stays plain — there is nothing to guess
    /// from, and prose in a fence should not acquire colours.
    #[test]
    fn a_fence_without_a_language_is_left_alone() {
        assert_eq!(highlight("", "a < b // not a comment"), "a &lt; b // not a comment");
    }

    /// Python's `"""` used to lex as an empty string followed by an
    /// unterminated one, which coloured the rest of the block.
    #[test]
    fn python_triple_quotes_are_one_string() {
        let out = highlight("python", "def f():\n    \"\"\"Doc \"with\" quotes.\"\"\"\n    return 1");
        assert!(out.contains("md2pdf-t-s\">\"\"\"Doc \"with\" quotes.\"\"\"<"), "{out}");
        assert!(out.contains("md2pdf-t-n\">1<"), "the code after it still lexes:\n{out}");
    }

    /// `//` inside a regex literal is not a comment. The lexer decides from
    /// what came before the slash.
    #[test]
    fn a_regex_literal_is_not_a_comment() {
        let out = highlight("js", "s.replace(/\\/\\//g, '') + 1");
        assert!(!out.contains("md2pdf-t-c"), "regex read as a comment:\n{out}");
        assert!(out.contains("md2pdf-t-n\">1<"), "the line ended early:\n{out}");
        // Division still divides.
        let div = highlight("js", "const r = a / b / c");
        assert!(!div.contains("md2pdf-t-s"), "division read as a regex:\n{div}");
    }

    #[test]
    fn numbers_keep_their_prefixes_separators_and_exponents() {
        for (code, want) in [
            ("0xFF", "0xFF"),
            ("1_000_000", "1_000_000"),
            ("1e-9", "1e-9"),
            ("1.5f32", "1.5f32"),
            ("0b1010", "0b1010"),
        ] {
            let out = highlight("rust", code);
            assert!(out.contains(&format!("md2pdf-t-n\">{want}<")), "{code}: {out}");
        }
    }

    #[test]
    fn only_screaming_snake_reads_as_a_constant() {
        assert!(highlight("rust", "const MAX_SIZE: u8 = 1;").contains("md2pdf-t-v\">MAX_SIZE<"));
        // A capitalised identifier is a table, a type or a class far more
        // often than it is a constant.
        for (lang, code, word) in [
            ("sql", "SELECT id FROM USERS", "USERS"),
            ("java", "class Foo { X x; }", "X"),
        ] {
            let out = highlight(lang, code);
            assert!(!out.contains(&format!("md2pdf-t-v\">{word}<")), "{lang}: {out}");
        }
    }

    #[test]
    fn a_called_name_reads_as_a_function() {
        let out = highlight("js", "function greet(who) { return format(who) }");
        assert!(out.contains("md2pdf-t-f\">greet<"), "{out}");
        assert!(out.contains("md2pdf-t-f\">format<"), "{out}");
        assert!(out.contains("md2pdf-t-k\">function<"), "{out}");
    }

    #[test]
    fn stylesheets_separate_selectors_from_properties() {
        let out = highlight("css", ".card > a:hover { color: #fff; margin: 1.5rem }");
        assert!(out.contains("md2pdf-t-t\">.card<"), "{out}");
        assert!(out.contains("md2pdf-t-p\">color<"), "{out}");
        assert!(out.contains("md2pdf-t-p\">margin<"), "{out}");
        assert!(out.contains("md2pdf-t-n\">1.5rem<"), "the unit rides along:\n{out}");
        assert!(highlight("css", "@media print { a { b: c } }").contains("md2pdf-t-m\">@media<"));
    }

    #[test]
    fn mapping_keys_are_told_apart_from_values() {
        let yaml = highlight("yaml", "title: md2pdf\nurl: https://example.com\nlist:\n  - a: 1");
        assert!(yaml.contains("md2pdf-t-p\">title<"), "{yaml}");
        assert!(yaml.contains("md2pdf-t-p\">a<"), "a key under a list marker:\n{yaml}");
        // The `https` in a value is not a key just because a colon follows.
        assert!(!yaml.contains("md2pdf-t-p\">https<"), "{yaml}");

        let json = highlight("json", "{\"name\": \"value\"}");
        assert!(json.contains("md2pdf-t-p\">\"name\"<"), "{json}");
        assert!(json.contains("md2pdf-t-s\">\"value\"<"), "{json}");
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
    /// Every language name the highlighter advertises, every lexer behind
    /// them, against input built to break each one. Not a character may be
    /// added, lost, reordered or left unescaped.
    ///
    /// This is the correctness test and the injection guard at once: the only
    /// way markup reaches the output is if some lexer emits source it did not
    /// escape, and that shows up here as a mismatch.
    #[test]
    fn highlighting_never_loses_source_text() {
        // Shapes chosen to leave a lexer mid-token: unterminated strings and
        // comments, delimiters inside literals, markup characters everywhere.
        const HOSTILE: &[&str] = &[
            "",
            "\n\n",
            "a < b > c & d \"quoted\" 'single'",
            "<script>alert(1)</script>",
            "\"unterminated",
            "'unterminated",
            "`unterminated",
            "/* unterminated",
            "// trailing\n",
            "#\n",
            "\"\"\"triple\"\"\" r\"raw\" r#\"hash\"#",
            "0x1F 1_000 1e-9 1.5f32 .5 1..2",
            "s.replace(/\\/\\//g, '')",
            "a/b/c",
            "@dec #attr $var --flag !bang",
            "{ } [ ] ( ) : ; , .",
            "key: value\n- item: 2",
            "\\ \\\\ \\\" \\n",
            "emoji 🚀 and 中文 and é",
            "-- sql\n% latex\n; lisp",
            "<!-- comment --> </>",
            "a{b:c}d",
        ];
        // Every name `lang_for` knows, plus the special lexers and the
        // fallback, so adding a language cannot skip this.
        const NAMES: &[&str] = &[
            "rust", "js", "ts", "python", "go", "c", "cpp", "java", "cs", "json", "yaml", "toml",
            "sh", "sql", "css", "scss", "typst", "diff", "php", "ruby", "swift", "lua", "r",
            "dart", "scala", "perl", "powershell", "dockerfile", "makefile", "graphql", "protobuf",
            "hcl", "nix", "zig", "elixir", "haskell", "latex", "julia", "html", "xml", "svg",
            "vue", "svelte", "patch", "brainfuck", "",
        ];
        for name in NAMES {
            for code in HOSTILE {
                let out = highlight(name, code);
                assert_eq!(
                    strip_tags(&out),
                    esc_text(code),
                    "lang {name:?} mangled {code:?}\ngot: {out}"
                );
            }
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
