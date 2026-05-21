// md2pdf — Markdown -> PDF, entirely inside Typst.
//
// `prepare()` feeds Markdown to the WASM engine (`engine.wasm`, the Rust/comrak
// "custom md engine"); the caller (`main.typ`, in the document's directory)
// does the final `eval`. All Markdown processing happens here; the host shim
// only does file/network I/O.

#import "styles/modern-tech.typ": article
#import "admonitions.typ": admonition, spoiler, task-item
// mitex + mmdr are vendored into the package so it is fully self-contained
// and offline — no @preview resolution needed (works in typst.ts too).
#import "vendor/mitex/lib.typ": mi, mitex
#import "vendor/mmdr/lib.typ": mermaid

#let _engine = plugin("engine.wasm")

// Helpers handed to the engine output via `eval` scope.
#let _md-math(display, src) = if display { mitex(src) } else { mi(src) }
#let _md-mermaid(code) = mermaid(code)

// Emoji are rendered as bundled Twemoji SVGs (package-relative, so they work
// in the CLI and — once the worker maps them into the VFS — in the browser).
#let _twemoji(cp) = box(baseline: 0.15em, height: 1em, image("twemoji/" + cp + ".svg"))

// True during the host shim's pass-1 `typst query` (remote-image discovery).
#let _querying = sys.inputs.at("md2pdf-query", default: none) != none

// Extract and YAML-decode a leading `---...---` frontmatter block.
#let _frontmatter(md) = {
  let lines = md.split("\n")
  if lines.len() == 0 or lines.at(0).trim() != "---" {
    (:)
  } else {
    let end = none
    for i in range(1, lines.len()) {
      if lines.at(i).trim() == "---" {
        end = i
        break
      }
    }
    if end == none {
      (:)
    } else {
      let decoded = yaml.decode(bytes(lines.slice(1, end).join("\n")))
      if type(decoded) == dictionary { decoded } else { (:) }
    }
  }
}

// Normalise a string-or-list value to a list.
#let _as-list(v) = if type(v) == str { (v,) } else if type(v) == array { v } else { () }

// Normalise a frontmatter author field (string or list) to a list.
#let _authors-of(fm) = _as-list(fm.at("authors", default: fm.at("author", default: ())))

// Collect the DIN 5008 letter-mode fields present in the frontmatter.
#let _letter-args(fm) = {
  let g(a, b) = fm.at(a, default: fm.at(b, default: none))
  let r = (:)
  let lr = g("letter-return", "letter_return")
  if lr != none { r.insert("letter-return", lr) }
  let lt = g("letter-to", "letter_to")
  if lt != none { r.insert("letter-to", _as-list(lt)) }
  let lf = g("letter-from", "letter_from")
  if lf != none { r.insert("letter-from", _as-list(lf)) }
  let ls = g("letter-subject", "letter_subject")
  if ls != none { r.insert("letter-subject", ls) }
  let ld = g("letter-date", "letter_date")
  if ld != none { r.insert("letter-date", ld) }
  r
}

// Remote-image manifest [(url, alias), ...], discovered by the engine.
#let _remotes(md) = {
  str(_engine.remotes(bytes(md)))
    .split("\n")
    .filter(l => l.trim() != "")
    .map(l => {
      let p = l.split("\t")
      (url: p.at(0), alias: p.at(1, default: ""))
    })
}

// Prepare a Markdown string for rendering.
//
// Returns `(skip, remotes, body, template, scope)`. The caller (`main.typ`)
// does the final `eval`, so `image()` paths resolve against the document root.
//
// Named `..opts` (title, authors, page-numbers) override frontmatter.
#let prepare(markdown, ..opts) = {
  let remotes = _remotes(markdown)
  if _querying {
    // Pass-1 query run: only the manifest is needed.
    (skip: true, remotes: remotes, body: "", template: none, scope: (:))
  } else {
    let fm = _frontmatter(markdown)
    let named = opts.named()

    // Title precedence: explicit opt > frontmatter > leading H1. When the
    // title comes from a leading H1, the engine drops that H1 from the body.
    let explicit-title = named.at("title", default: fm.at("title", default: ""))
    let h1 = str(_engine.leading_h1(bytes(markdown)))
    let from-h1 = explicit-title == "" and h1 != ""
    let title = if explicit-title != "" { explicit-title } else { h1 }

    (
      skip: false,
      remotes: remotes,
      body: str(_engine.convert(bytes(markdown), bytes(if from-h1 { "1" } else { "" }))),
      template: article.with(
        title: title,
        authors: named.at("authors", default: _authors-of(fm)),
        page-numbers: named.at(
          "page-numbers",
          default: fm.at("pageNumbers", default: fm.at("page-numbers", default: true)),
        ),
        .._letter-args(fm),
      ),
      scope: (
        admonition: admonition,
        spoiler: spoiler,
        task-item: task-item,
        md-math: _md-math,
        md-mermaid: _md-mermaid,
        twemoji: _twemoji,
      ),
    )
  }
}
