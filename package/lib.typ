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
#import "vendor/mmdr/lib.typ": mermaid, mermaid-svg

// One plugin instance for the whole package — `tokens.typ` owns it so the
// palette is decoded once and the engine is loaded once.
#import "tokens.typ": engine as _engine, tokens
#import "bibliography-styles.typ": bibliography-styles

// Helpers handed to the engine output via `eval` scope.
#let _md-math(display, src) = if display { mitex(src) } else { mi(src) }
// A formula the engine found mitex cannot typeset: its source, in the colour
// the HTML output uses for the same fallback.
#let _md-math-error(src) = text(fill: rgb(tokens.admonition.danger.accent), raw(src))
#let _md-jump(id, body) = context {
  let targets = query(label(id))
  if targets.len() > 0 { link(targets.first().location(), body) } else { body }
}
#let _md-mermaid(code) = mermaid(code)
// A raw `<kbd>` key: monospace on a light key cap.
#let _md-kbd(body) = box(
  fill: luma(245),
  stroke: 0.5pt + luma(180),
  inset: (x: 0.25em),
  outset: (y: 0.2em),
  radius: 2pt,
  text(font: ("DejaVu Sans Mono",), size: 0.85em, body),
)
// Typst accepts one label per cite, so adjacent IEEE cites own separate
// brackets. Keep CSL's numbering and punctuation, but let Markdown own these.
#let _md-cite-group(keys) = {
  show cite: it => {
    show regex("[\\[\\]]"): none
    it
  }
  [\[]
  for key in keys { cite(key) }
  [\]]
}

// Emoji are rendered as bundled Twemoji SVGs (package-relative, so they work
// in the CLI and — once the worker maps them into the VFS — in the browser).
#let _twemoji(cp) = box(baseline: 0.15em, height: 1em, image("twemoji/" + cp + ".svg"))

// Stands in for an image that cannot be loaded, exactly like the blank file a
// host writes for a remote image whose fetch failed.
#let _blank-image = bytes("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"></svg>")
#let _missing-image(..args) = image(_blank-image, format: "svg", ..args.named())

// Wrap a host's `asset` loader so the paths it reported missing get the blank
// stand-in instead of an `image()` call that would abort the compile.
#let _guarded-asset(asset, missing) = (path, ..args) => {
  if path in missing { _missing-image(..args) } else { asset(path, ..args) }
}

// The images a document references, from one engine parse: `remotes` pairs
// each remote URL with its alias under `remote-dir`, `images` lists the local
// paths for a host to check before it renders — the ones it cannot supply go
// back in as `missing-images`.
#let _discover(md, remote-dir) = {
  let remotes = ()
  let images = ()
  for line in str(_engine.html_assets(bytes(md))).split("\n") {
    let p = line.split("\t")
    if p.at(0) == "image" {
      images.push(p.at(1))
    } else if p.at(0) == "remote" {
      // The engine names the default `remote/` directory.
      remotes.push((url: p.at(1), alias: remote-dir + p.at(2).slice("remote".len())))
    }
  }
  (remotes: remotes, images: images)
}

// True during the host shim's pass-1 `typst query` (remote-image discovery).
#let _querying = sys.inputs.at("md2pdf-query", default: none) != none

// The frontmatter block, read by the engine: keys are lowercased with `_` read
// as `-`, and every scalar is a string except `true`/`false` and null.
#let _frontmatter(md) = json(_engine.frontmatter(bytes(md)))

// Any frontmatter value as display text, or `none`. A list joins with commas.
#let _text-of(v) = {
  if v == none or type(v) == dictionary { none } else if type(v) == array {
    v.map(_text-of).filter(t => t != none).join(", ")
  } else if type(v) == bool { if v { "true" } else { "false" } } else { str(v) }
}

// Normalise a string-or-list value to a list of strings.
#let _as-list(v) = {
  if type(v) == array { v.map(_text-of).filter(t => t != none) } else if v == none { () } else {
    (_text-of(v),)
  }
}

// `false`, `no` and `off` all mean no, whichever way YAML spelled them.
#let _is-off(v) = v == false or (type(v) == str and lower(v.trim()) in ("false", "no", "off"))
#let _is-on(v) = v == true or (type(v) == str and lower(v.trim()) in ("true", "yes", "on"))

// Normalise a frontmatter author field (string or list) to a list.
#let _authors-of(fm) = _as-list(fm.at("authors", default: fm.at("author", default: none)))

// Collect the DIN 5008 letter-mode fields present in the frontmatter.
#let _letter-args(fm) = {
  let r = (:)
  for key in ("letter-return", "letter-subject", "letter-date") {
    let v = _text-of(fm.at(key, default: none))
    if v != none { r.insert(key, v) }
  }
  for key in ("letter-to", "letter-from") {
    let v = fm.at(key, default: none)
    if v != none { r.insert(key, _as-list(v)) }
  }
  r
}

// A positive `pt`/`mm`/`cm`/`in` length, or `none` — also for a value that is
// not one, so a typo falls back to the default height.
#let _length-of(fm, key) = {
  let raw = _text-of(fm.at(key, default: none))
  if raw == none { return none }
  raw = raw.trim()
  let unit = (("pt", 1pt), ("mm", 1mm), ("cm", 1cm), ("in", 1in)).find(
    pair => raw.ends-with(pair.at(0)),
  )
  if unit == none { return none }
  let number = raw.slice(0, raw.len() - unit.at(0).len())
  let valid = number != "" and number != "." and number.split(".").len() <= 2 and number.clusters().all(
    char => "0123456789.".contains(char),
  )
  if not valid { return none }
  let length = float(number) * unit.at(1)
  if length <= 0pt { none } else { length }
}

// Frontmatter `lang`, as `(lang, region)`. Accepts "de", "de-AT", "de_DE" and
// "zh-Hans" (a script subtag is not a region). Typst localises `outline(title:
// auto)` and friends from this, so `[toc]` becomes "Inhaltsverzeichnis" under
// `lang: de`. Anything that is not a language code falls back to English.
#let _lang-of(fm) = {
  let raw = _text-of(fm.at("lang", default: none))
  let parts = if raw == none { () } else { raw.trim().split(regex("[-_]")) }
  let letters(s) = s.clusters().all(c => lower(c) != upper(c) and c.len() == 1)
  let lang = if parts.len() > 0 { lower(parts.at(0)) } else { "" }
  if lang.len() not in (2, 3) or not letters(lang) { return ("en", none) }
  let region = parts.at(1, default: "")
  (lang, if region.len() == 2 and letters(region) { upper(region) } else { none })
}

// Frontmatter `date`, as a display string.
#let _date-of(fm) = _text-of(fm.at("date", default: none))

// Collect the running header/footer slots present in the frontmatter.
#let _furniture-args(fm) = {
  let r = (:)
  for edge in ("header", "footer") {
    for side in ("left", "center", "right") {
      let key = edge + "-" + side
      let v = _text-of(fm.at(key, default: none))
      if v != none { r.insert(key, v) }
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
    let v = fm.at(key, default: none)
    if key == "cover" and _is-off(v) { continue }
    if key == "cover" and _is-on(v) { v = true }
    if v != none and v != false {
      r.insert(key, if type(v) == bool { v } else { _text-of(v) })
    }
  }
  r
}

// ---------------------------------------------------------------------------
// HTML output
// ---------------------------------------------------------------------------
//
// The engine renders HTML itself — styling, outline, math and highlighting all
// live in Rust, so the CLI and the browser emit the same bytes. Typst's only
// job here is the I/O the engine cannot do: read the referenced assets and run
// the Mermaid plugin. Everything is handed back as one blob plus a
// `key<TAB>byte-length` manifest.

#let _lines(raw) = {
  str(raw).split("\n").filter(l => l.trim() != "")
}

// Collect every asset the document needs, skipping the ones we cannot load so
// one missing file degrades to a placeholder instead of failing the build.
//
// `html_assets` answers for images, remotes, emoji, fonts and diagrams from a
// single parse; asking each question separately parsed the document five times.
#let _html-assets(md, read-asset, remote-dir, missing) = {
  let items = ()
  for line in _lines(_engine.html_assets(bytes(md))) {
    let p = line.split("\t")
    let kind = p.at(0)
    if kind == "image" {
      if p.at(1) not in missing { items.push((key: p.at(1), data: read-asset(p.at(1)))) }
    } else if kind == "remote" {
      // The engine lists remotes under the default `remote/` directory.
      let key = remote-dir + p.at(2).slice("remote".len())
      items.push((key: key, data: read-asset(key)))
    } else if kind == "emoji" {
      let name = "twemoji/" + p.at(1) + ".svg"
      items.push((key: name, data: read(name, encoding: none)))
    } else if kind == "font" {
      // Only a document with math asks for these, and the alphanumerics face
      // only once a formula reaches into that block.
      items.push((key: p.at(1), data: read(p.at(1), encoding: none)))
    } else if kind == "mermaid" {
      let code = p.at(2, default: "").replace("\\n", "\n").replace("\\\\", "\\")
      items.push((key: p.at(1), data: bytes(mermaid-svg(code))))
    }
  }
  items = items.filter(it => it.data != none)
  (
    manifest: items.fold("", (acc, it) => acc + it.key + "\t" + str(it.data.len()) + "\n"),
    blob: items.fold(bytes(()), (acc, it) => acc + it.data),
  )
}

// Render Markdown to a self-contained HTML document.
//
// `read-asset` must be a closure defined in the calling file — `read()` inside
// a package resolves against the package root, not the document root, exactly
// like `asset` in `prepare()`. It should return `none` for a path it cannot
// read, so a missing image does not fail the whole document.
//
// `remote-dir` and `missing-images` are as in `prepare()`.
#let prepare-html(
  markdown,
  read-asset: p => none,
  standalone: true,
  theme: "system",
  remote-dir: "remote",
  missing-images: (),
) = {
  let assets = _html-assets(markdown, read-asset, remote-dir, missing-images)
  str(_engine.render_html(
    bytes(markdown),
    bytes(
      "standalone=" + if standalone { "1" } else { "0" }
        + "\ntheme=" + theme
        + "\nremote-dir=" + remote-dir,
    ),
    bytes(assets.manifest),
    assets.blob,
  ))
}

// Prepare a Markdown string for rendering.
//
// Returns `(skip, remotes, images, unknown-cites, body, template, scope)`. The
// caller (`main.typ`) does the final `eval`, so `image()` paths resolve against
// the document root. `images` lists the local image paths the document uses,
// `unknown-cites` the citation keys its inline bibliography lacks.
//
// Named `..opts`: `title` / `authors` override frontmatter, `page-numbers` is a
// default frontmatter can override, and `asset` loads a document-relative image
// path. `asset` must be a closure defined in the calling file — an `image()`
// call written inside this package would resolve against the package root, not
// the document root. `remote-dir` is the directory the host fetches remote
// images into; every `<remote-dir>/<hash>` alias lives under it.
// `missing-images` lists local paths the host found it cannot supply; each
// renders as a blank stand-in instead of failing the compile.
#let prepare(markdown, remote-dir: "remote", missing-images: (), ..opts) = {
  let (remotes, images) = _discover(markdown, remote-dir)
  let fm = _frontmatter(markdown)
  let bibliography-mode = _text-of(fm.at("bibliography", default: none))
  let inline-bibliography = bibliography-mode != none and lower(bibliography-mode.trim()) == "inline"
  // Cited keys the inline bibliography lacks; they stay text, and a host can
  // warn about them.
  let unknown-cites = if inline-bibliography {
    str(_engine.unknown_citations(bytes(markdown))).split("\n").filter(k => k != "")
  } else { () }
  if _querying {
    // Pass-1 query run: only the manifests are needed.
    (
      skip: true,
      remotes: remotes,
      images: images,
      unknown-cites: unknown-cites,
      body: "",
      template: none,
      scope: (:),
    )
  } else {
    let asset = _guarded-asset(opts.named().at("asset", default: image), missing-images)
    let named = opts.named()
    let doc-lang = _lang-of(fm)
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
    let bibliography-style = _text-of(fm.at("bibliography-style", default: none))
    if bibliography-style not in bibliography-styles { bibliography-style = "ieee" }
    let language = if doc-lang.at(0) == "de" { "de" } else { "en" }
    let bibliography-title = {
      let v = fm.at("bibliography-title", default: none)
      if v == none { tokens.strings.references.at(language) } else if _is-off(v) { none } else {
        let t = _text-of(v)
        if t == none or t.trim() == "" { none } else { t }
      }
    }

    // Title precedence: explicit opt > frontmatter > leading H1. When the
    // title comes from a leading H1, the engine drops that H1 from the body.
    let explicit-title = named.at("title", default: _text-of(fm.at("title", default: none)))
    if explicit-title == none { explicit-title = "" }
    let h1 = str(_engine.leading_h1(bytes(source)))
    let from-h1 = explicit-title == "" and h1 != ""
    let title = if explicit-title != "" { explicit-title } else { h1 }

    (
      skip: false,
      remotes: remotes,
      images: images,
      unknown-cites: unknown-cites,
      body: str(_engine.convert(
        bytes(source),
        bytes(if from-h1 { "1" } else { "" }),
        bytes(bib),
        bytes(remote-dir),
      )) + if bib != "" { "\n\n#md-bibliography()" } else { "" },
      template: article.with(
        title: title,
        authors: named.at("authors", default: _authors-of(fm)),
        date: _date-of(fm),
        lang: doc-lang.at(0),
        region: doc-lang.at(1),
        remotes: remotes,
        asset: asset,
        // The document declares its own layout: frontmatter beats the caller's
        // value, which is only a default.
        page-numbers: {
          let v = fm.at("pagenumbers", default: fm.at("page-numbers", default: none))
          if v == none { named.at("page-numbers", default: true) } else if _is-off(v) { false } else if _is-on(
            v,
          ) { true } else { _text-of(v) }
        },
        .._letter-args(fm),
        .._furniture-args(fm),
        .._cover-args(fm),
      ),
      scope: (
        admonition: admonition.with(lang: doc-lang.at(0)),
        spoiler: spoiler,
        task-item: task-item,
        md-math: _md-math,
        md-math-error: _md-math-error,
        md-jump: _md-jump,
        md-image: asset,
        md-missing-image: _missing-image,
        md-mermaid: _md-mermaid,
        md-kbd: _md-kbd,
        md-cite-group: _md-cite-group,
        twemoji: _twemoji,
        md-bibliography: () => bibliography(
          bytes(bib),
          style: bibliography-style,
          title: bibliography-title,
        ),
      ),
    )
  }
}
