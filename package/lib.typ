// md2pdf — Markdown -> PDF or HTML, entirely inside Typst.
//
// `prepare()` feeds Markdown to the WASM engine (`engine.wasm`, the Rust/comrak
// "custom md engine"); the caller (`main.typ`, in the document's directory)
// does the final `eval`. All Markdown processing happens here; the host shim
// only does file/network I/O.

#import "styles/modern-tech.typ": article as pdf-article, md-align as pdf-md-align, md-row as pdf-md-row, md-task-list as pdf-md-task-list, md-rule as pdf-md-rule, md-toc as pdf-md-toc, md-pagebreak as pdf-md-pagebreak, md-blank as pdf-md-blank, md-image as pdf-md-image
#import "styles/modern-web.typ": article as web-article, md-align as web-md-align, md-row as web-md-row, md-task-list as web-md-task-list, md-rule as web-md-rule, md-toc as web-md-toc, md-pagebreak as web-md-pagebreak, md-blank as web-md-blank, md-image as web-md-image, admonition as web-admonition, spoiler as web-spoiler, task-item as web-task-item
#import "admonitions.typ": admonition as pdf-admonition, spoiler as pdf-spoiler, task-item as pdf-task-item
// mitex + mmdr are vendored into the package so it is fully self-contained
// and offline — no @preview resolution needed (works in typst-wasm too).
#import "vendor/mitex/lib.typ": mi, mitex
#import "vendor/mmdr/lib.typ": mermaid

#let _engine = plugin("engine.wasm")

// Helpers handed to the engine output via `eval` scope.
#let _md-math(display, src) = if display { mitex(src) } else { mi(src) }
#let _md-mermaid(code) = context if target() == "html" {
  set image(alt: "Mermaid diagram")
  mermaid(code)
} else {
  mermaid(code)
}

#let _article(..args) = context if target() == "html" {
  web-article(..args)
} else {
  pdf-article(..args)
}
#let _admonition(..args) = context if target() == "html" {
  web-admonition(..args)
} else {
  pdf-admonition(..args)
}
#let _spoiler(..args) = context if target() == "html" { web-spoiler(..args) } else { pdf-spoiler(..args) }
#let _task-item(..args) = context if target() == "html" { web-task-item(..args) } else { pdf-task-item(..args) }
#let _md-align(..args) = context if target() == "html" { web-md-align(..args) } else { pdf-md-align(..args) }
#let _md-row(..args) = context if target() == "html" { web-md-row(..args) } else { pdf-md-row(..args) }
#let _md-task-list(..args) = context if target() == "html" {
  web-md-task-list(web-task-item, ..args)
} else {
  pdf-md-task-list(pdf-task-item, ..args)
}
#let _md-rule(..args) = context if target() == "html" { web-md-rule(..args) } else { pdf-md-rule(..args) }
#let _md-toc(lang: "en", ..args) = context if target() == "html" {
  web-md-toc(lang: lang, ..args)
} else {
  pdf-md-toc(..args)
}
#let _md-pagebreak(..args) = context if target() == "html" { web-md-pagebreak(..args) } else { pdf-md-pagebreak(..args) }
#let _md-blank(..args) = context if target() == "html" { web-md-blank(..args) } else { pdf-md-blank(..args) }
#let _md-image(asset, ..args) = context if target() == "html" {
  web-md-image(asset, ..args)
} else {
  pdf-md-image(asset, ..args)
}
#let _md-bibliography(bib, style, title) = context if target() == "html" {
  html.h2(title)
  bibliography(bytes(bib), style: style, title: none)
} else {
  bibliography(bytes(bib), style: style, title: title)
}

// Emoji are rendered as bundled Twemoji SVGs (package-relative, so they work
// in the CLI and — once the worker maps them into the VFS — in the browser).
#let _twemoji(cp) = box(baseline: 0.15em, height: 1em, image("twemoji/" + cp + ".svg"))
#let _twemoji-html(cp) = html.span(class: "twemoji", image(
  "twemoji/" + cp + ".svg",
  height: 1em,
  alt: "",
))
#let _target-twemoji(cp) = context if target() == "html" { _twemoji-html(cp) } else { _twemoji(cp) }

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
      let decoded = yaml(bytes(lines.slice(1, end).join("\n")))
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

// Any scalar frontmatter value as a display string. YAML hands back a datetime
// for unquoted dates, which `str()` cannot take.
#let _text-of(v) = if type(v) == datetime { v.display() } else { str(v) }

#let _length-of(fm, key) = {
  let value = fm.at(key, default: fm.at(key.replace("-", "_"), default: none))
  if value == none { return none }

  let raw = _text-of(value).trim()
  let unit = (("pt", 1pt), ("mm", 1mm), ("cm", 1cm), ("in", 1in)).find(
    pair => raw.ends-with(pair.at(0)),
  )
  if unit == none {
    panic(key + " must be a positive length using pt, mm, cm, or in")
  }

  let number = raw.slice(0, raw.len() - unit.at(0).len())
  let parts = number.split(".")
  let valid = number != "" and number != "." and parts.len() <= 2 and number.clusters().all(
    char => "0123456789.".contains(char),
  )
  if not valid {
    panic(key + " must be a positive length using pt, mm, cm, or in")
  }

  let length = float(number) * unit.at(1)
  if length <= 0pt {
    panic(key + " must be greater than zero")
  }
  length
}

// Frontmatter `lang`, as `(lang, region)`. Accepts "de" or "de-AT"; the region
// is optional. Typst localises `outline(title: auto)` and friends from this, so
// `[toc]` becomes "Inhaltsverzeichnis" under `lang: de`.
#let _lang-of(fm) = {
  let raw = fm.at("lang", default: "en")
  let parts = _text-of(raw).trim().split("-")
  let lang = lower(parts.at(0))
  if lang == "" { lang = "en" }
  (lang, if parts.len() > 1 { upper(parts.at(1)) } else { none })
}

// Frontmatter `date`, as a display string.
#let _date-of(fm) = {
  let d = fm.at("date", default: none)
  if d == none { none } else { _text-of(d) }
}

// Collect the running header/footer slots present in the frontmatter.
#let _furniture-args(fm) = {
  let r = (:)
  for edge in ("header", "footer") {
    for side in ("left", "center", "right") {
      let key = edge + "-" + side
      let v = fm.at(key, default: fm.at(edge + "_" + side, default: none))
      if v != none { r.insert(key, _text-of(v)) }
    }
  }
  for key in ("header-height", "footer-height") {
    let value = _length-of(fm, key)
    if value != none { r.insert(key, value) }
  }
  r
}

// Collect the cover-page fields present in the frontmatter.
#let _cover-args(fm) = {
  let r = (:)
  let keys = (
    "cover",
    "cover-color",
    "cover-subtitle",
    "cover-logo",
    "cover-date",
    "cover-image",
    "cover-text-color",
  )
  for key in keys {
    let v = fm.at(key, default: fm.at(key.replace("-", "_"), default: none))
    if v != none { r.insert(key, if type(v) == bool { v } else { _text-of(v) }) }
  }
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
// Named `..opts`: `title` / `authors` override frontmatter, `page-numbers` is a
// default frontmatter can override, and `asset` loads a document-relative image
// path. `asset` must be a closure defined in the calling file — an `image()`
// call written inside this package would resolve against the package root, not
// the document root.
#let prepare(markdown, ..opts) = {
  let remotes = _remotes(markdown)
  if _querying {
    // Pass-1 query run: only the manifest is needed.
    (skip: true, remotes: remotes, body: "", template: none, scope: (:))
  } else {
    let fm = _frontmatter(markdown)
    let named = opts.named()
    let doc-lang = _lang-of(fm)
    let bibliography-mode = fm.at("bibliography", default: none)
    let inline-bibliography = (
      type(bibliography-mode) == str and lower(bibliography-mode.trim()) == "inline"
    )
    let bib = if inline-bibliography {
      str(_engine.inline_bibliography(bytes(markdown)))
    } else {
      ""
    }
    let source = if bib != "" {
      str(_engine.without_inline_bibliography(bytes(markdown)))
    } else {
      markdown
    }
    let bibliography-style = _text-of(fm.at(
      "bibliography-style",
      default: fm.at("bibliography_style", default: "ieee"),
    ))
    let bibliography-title = if doc-lang.at(0) == "de" { [Referenzen] } else { [References] }

    // Title precedence: explicit opt > frontmatter > leading H1. When the
    // title comes from a leading H1, the engine drops that H1 from the body.
    let explicit-title = named.at("title", default: fm.at("title", default: ""))
    let h1 = str(_engine.leading_h1(bytes(source)))
    let from-h1 = explicit-title == "" and h1 != ""
    let title = if explicit-title != "" { explicit-title } else { h1 }

    (
      skip: false,
      remotes: remotes,
      body: str(_engine.convert(
        bytes(source),
        bytes(if from-h1 { "1" } else { "" }),
        bytes(if bib != "" { "1" } else { "" }),
      )) + if bib != "" { "\n\n#md-bibliography()" } else { "" },
      template: _article.with(
        title: title,
        authors: named.at("authors", default: _authors-of(fm)),
        date: _date-of(fm),
        lang: doc-lang.at(0),
        region: doc-lang.at(1),
        remotes: remotes,
        asset: named.at("asset", default: image),
        // The document declares its own layout: frontmatter beats the caller's
        // value (the web app's page-numbers toggle is a default, not an override).
        page-numbers: fm.at(
          "pageNumbers",
          default: fm.at(
            "page-numbers",
            default: named.at("page-numbers", default: true),
          ),
        ),
        .._letter-args(fm),
        .._furniture-args(fm),
        .._cover-args(fm),
      ),
      scope: (
        admonition: _admonition.with(lang: doc-lang.at(0)),
        spoiler: _spoiler,
        task-item: _task-item,
        md-align: _md-align,
        md-row: _md-row,
        md-task-list: _md-task-list,
        md-rule: _md-rule,
        md-toc: _md-toc.with(lang: doc-lang.at(0)),
        md-pagebreak: _md-pagebreak,
        md-blank: _md-blank,
        md-image: _md-image.with(named.at("asset", default: image)),
        md-math: _md-math,
        md-mermaid: _md-mermaid,
        twemoji: _target-twemoji,
        md-bibliography: () => _md-bibliography(bib, bibliography-style, bibliography-title),
      ),
    )
  }
}
