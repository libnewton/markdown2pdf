// Running header / footer helpers.
//
// A slot value is either free text with `{placeholder}`s or a Markdown image
// (`![](path =WxH)`). The Markdown form is deliberate: the engine's remote-image
// scan runs over the raw source — frontmatter included — so a remote logo is
// prefetched by the host exactly like a body image.

// `=200x120`, `=200x`, `=x120` -> `(width, height)` in pt, or `none`.
#let _parse-dims(raw) = {
  let s = raw.trim().trim("=", at: start, repeat: false)
  let x = s.position("x")
  if x == none { x = s.position("X") }
  if x == none { return none }
  let num(part) = {
    if part == "" { return none }
    for c in part.clusters() {
      if not "0123456789.".contains(c) { return "bad" }
    }
    float(part) * 1pt
  }
  let w = num(s.slice(0, x))
  let h = num(s.slice(x + 1))
  if w == "bad" or h == "bad" or (w == none and h == none) { return none }
  (w, h)
}

// Parse `![alt](path =WxH)`. Returns `(path, width, height)` or `none`.
#let parse-image-field(value) = {
  let v = value.trim()
  if not v.starts-with("![") or not v.ends-with(")") { return none }
  let open = v.position("](")
  if open == none { return none }
  let inner = v.slice(open + 2, v.len() - 1).trim()
  if inner == "" { return none }

  let parts = inner.split(" ").filter(p => p != "")
  if parts.len() > 1 {
    let dims = _parse-dims(parts.last())
    if dims != none {
      return (
        path: parts.slice(0, parts.len() - 1).join(" "),
        width: dims.at(0),
        height: dims.at(1),
      )
    }
  }
  (path: inner, width: none, height: none)
}

// Map an http(s) URL onto the `remote/<hash>` alias the engine assigned it. The
// manifest comes from `prepare()`, so the hash function lives in exactly one
// place.
#let resolve-path(path, remotes) = {
  for r in remotes {
    if r.url == path { return r.alias }
  }
  path
}

// Build the `image()` arguments for a parsed field, defaulting the size.
#let image-args(img, fallback-height) = {
  if img.width == none and img.height == none { return (height: fallback-height) }
  let a = (:)
  if img.width != none { a.insert("width", img.width) }
  if img.height != none { a.insert("height", img.height) }
  a
}

// Substitute `{page}`, `{pages}` and the caller's variables into a template
// string. Unknown placeholders are left verbatim rather than raising.
#let subst(template, vars) = context {
  let values = (
    page: str(counter(page).get().first()),
    pages: str(counter(page).final().first()),
    ..vars,
  )
  template.replace(
    regex("\\{([a-z-]+)\\}"),
    m => values.at(m.captures.at(0), default: m.text),
  )
}

// One header/footer cell: an image, substituted text, or nothing. `asset` loads
// a document-relative path — see `prepare()` for why it cannot be `image`.
#let slot(value, vars, remotes, asset, image-height: 5mm) = {
  if value == none or value == "" { return none }
  let img = parse-image-field(value)
  if img == none { return subst(value, vars) }
  box(asset(resolve-path(img.path, remotes), ..image-args(img, image-height)))
}

// The three-column bar shared by header and footer. Small, grey, no rule.
#let bar(left-slot, center-slot, right-slot, vars, remotes, asset) = {
  let cells = (left-slot, center-slot, right-slot).map(v => slot(v, vars, remotes, asset))
  if cells.all(c => c == none) { return none }
  block(width: 100%, spacing: 0pt, {
    set text(size: 8.5pt, fill: luma(140))
    set par(justify: false, leading: 0.5em)
    grid(
      columns: (1fr, auto, 1fr),
      align: (left + horizon, center + horizon, right + horizon),
      column-gutter: 1em,
      ..cells.map(c => if c == none { [] } else { c }),
    )
  })
}
