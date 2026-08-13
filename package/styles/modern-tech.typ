// Modern Tech style.
// Sans-serif throughout (web-like reading), paragraph spacing, no first-line indent, modern code blocks.

#import "../admonitions.typ": admonition, spoiler, task-item, md2pdf-list-markers, md2pdf-enum-numbering
#import "../tokens.typ": tokens
#import "../furniture.typ": bar
#import "../cover.typ": cover-page

#let article(title: "", authors: (), ..args, body) = {
  let page-numbers = args.at("page-numbers", default: true)
  let date = args.at("date", default: none)
  let lang = args.at("lang", default: "en")
  let region = args.at("region", default: none)
  let remotes = args.at("remotes", default: ())
  let asset = args.at("asset", default: image)
  let header-height = args.at("header-height", default: none)
  let footer-height = args.at("footer-height", default: none)
  let letter-return = args.at("letter-return", default: "")
  let letter-to = args.at("letter-to", default: ())
  let letter-from = args.at("letter-from", default: ())
  let letter-subject = args.at("letter-subject", default: "")
  let letter-date = args.at("letter-date", default: "")
  let letter-mode = (
    letter-return != "" or letter-to.len() > 0
      or letter-from.len() > 0 or letter-subject != ""
      or letter-date != ""
  )

  // Cover page. Mutually exclusive with letter mode: a cover would push the
  // DIN 5008 address field to page 2 and break the envelope-window alignment.
  let cover-style = args.at("cover", default: none)
  let cover-image = args.at("cover-image", default: none)
  // A cover image is enough on its own — `cover: true` alongside it is optional.
  let cover-mode = (
    (cover-style != none and cover-style != false) or cover-image != none
  ) and not letter-mode
  let cover-subtitle = args.at("cover-subtitle", default: "")
  let cover-date = args.at("cover-date", default: if date == none { "" } else { date })

  // Header/footer slots. `page-numbers` supplies the default footer-center:
  // `true`/"1" -> "{page}", "1/1" -> "{page} / {pages}", any other string is
  // used verbatim as a template.
  let number-template = if page-numbers == false {
    none
  } else if page-numbers == true or page-numbers == "1" {
    "{page}"
  } else if page-numbers == "1/1" {
    "{page} / {pages}"
  } else if type(page-numbers) == str {
    page-numbers
  } else {
    "{page}"
  }
  let slot-of(name, fallback: none) = args.at(name, default: fallback)
  let header-slots = (slot-of("header-left"), slot-of("header-center"), slot-of("header-right"))
  let custom-footer-center = slot-of("footer-center")
  let footer-slots = (
    slot-of("footer-left"),
    if custom-footer-center == none { number-template } else { custom-footer-center },
    slot-of("footer-right"),
  )
  let author-line = if authors.len() > 0 { authors.join(", ") } else { "" }
  let vars = (
    title: title,
    subtitle: cover-subtitle,
    author: author-line,
    authors: author-line,
    date: if date == none { "" } else { date },
  )

  // 1) Page setup: wide margins for readability
  // Letter mode uses 20mm x-margin (DIN 5008 body margin).
  let page-margin-x = if letter-mode { 20mm } else { 1.8cm }
  let page-margin-y = 2cm
  let top-margin = if header-height == none {
    page-margin-y
  } else {
    calc.max(page-margin-y, header-height + 6mm)
  }
  let bottom-margin = if footer-height == none {
    page-margin-y
  } else {
    calc.max(page-margin-y, footer-height + 6mm)
  }
  set page(
    paper: "a4",
    margin: (left: page-margin-x, right: page-margin-x, top: top-margin, bottom: bottom-margin),
    numbering: if page-numbers != false { "1" } else { none },
    header-ascent: 6mm,
    footer-descent: 6mm,
    // A cover stays clean. Without one, the document starts on page one and
    // its page number belongs there even though the running head starts later.
    header: context {
      if here().page() > 1 { bar(..header-slots, vars, remotes, asset, height: header-height) }
    },
    footer: context {
      if here().page() > 1 {
        bar(..footer-slots, vars, remotes, asset, height: footer-height)
      } else if not cover-mode and custom-footer-center == none {
        bar(none, number-template, none, vars, remotes, asset, height: footer-height)
      }
    },
  )
  set document(title: title, author: authors, date: none)

  // 2) Font stack: a Latin sans for text and numbers, then a CJK face for the
  // characters it has no glyphs for. Typst falls through per character, so the
  // CJK face only ever paints what the Latin one cannot.
  let body-size = 10.5pt
  set text(
    font: (
      "IBM Plex Sans",
      "Roboto",
      "Libertinus Sans",
      "Noto Sans SC",
      "Noto Sans KR",
      "Noto Sans CJK SC",
      "Source Han Sans SC",
    ),
    size: body-size,
    // Drives hyphenation, smart quotes, and Typst's localised titles — a
    // `[toc]` renders as "Inhaltsverzeichnis" under `lang: de`.
    lang: lang,
    region: region,
  )

  // 3) Paragraphs: no first-line indent, paragraph-spacing mode (closer to web reading)
  set par(
    justify: true,
    leading: 1em,
    first-line-indent: 0pt,
    spacing: 1.2em,
  )
  set list(indent: 1em, body-indent: 0.5em, spacing: 0.8em, marker: md2pdf-list-markers)
  set enum(indent: 1em, body-indent: 0.5em, spacing: 0.8em, full: true, numbering: md2pdf-enum-numbering)

  // 4) Headings: explicit size ramp per level (clear hierarchy).
  // Typst's own defaults stop scaling at level 3, so `###` and `####` both
  // landed on body size and were indistinguishable. Every level therefore
  // carries its own size here. Levels 5-6 have no size headroom left above
  // body text and change register via letter tracking instead.
  // Headings keep the softer #333333 grey, which is lighter than the pure
  // black of a bold paragraph — so size carries the hierarchy on its own.
  // Spacing scales with the level too — a `####` needs far less air than a
  // `#`. Typst collapses adjacent block spacing to the larger of the two
  // values, so `below` bottoms out at `par.spacing` (1.2em) no matter what.
  // Sizes are multiples of `body-size` rather than `em`: Typst's built-in
  // per-level sizing still wraps our output (1.4x on an `#`, 1.2x on a `##`,
  // nothing from level 3 down — exactly the ramp being replaced), and an `em`
  // here would resolve against *that* and compound.
  let heading-styles = (
    (size: 1.52, tracking: 0pt, above: 1.9em, below: 1.3em),
    (size: 1.30, tracking: 0pt, above: 1.7em, below: 1.2em),
    (size: 1.16, tracking: 0pt, above: 1.5em, below: 1.2em),
    (size: 1.07, tracking: 0.02em, above: 1.35em, below: 1.2em),
    (size: 1.00, tracking: 0.05em, above: 1.3em, below: 1.2em),
    (size: 0.94, tracking: 0.08em, above: 1.25em, below: 1.2em),
  )
  show heading: it => {
    let style = heading-styles.at(calc.min(it.level, heading-styles.len()) - 1)
    let marker = if it.numbering != none {
      counter(heading).display(it.numbering) + h(0.5em)
    }
    block(
      above: style.above,
      below: style.below,
      text(
        weight: "bold",
        fill: rgb(tokens.base.heading),
        font: ("IBM Plex Sans", "Roboto"),
        size: body-size * style.size,
        tracking: style.tracking,
        marker + it.body,
      ),
    )
  }

  // 5) Link colour: tech blue
  show link: set text(fill: rgb(tokens.base.accent))
  show cite: it => {
    show regex("[0-9]+"): set text(fill: rgb(tokens.base.accent))
    it
  }

  // 6) Blockquotes: left accent line + light background
  set quote(block: true)
  show quote: it => {
    set par(first-line-indent: 0pt)
    block(
      fill: luma(248),
      stroke: (left: 2pt + rgb(tokens.base.accent)),
      inset: (left: 0.9em, right: 0.9em, top: 0.7em, bottom: 0.7em),
      radius: 6pt,
      width: 100%,
      it.body,
    )
  }

  // 7) Inline code: light background + rounded corners
  // Typst renders `raw` at 0.8 of the surrounding size, which leaves code
  // visibly smaller than the prose; the size below is absolute because an `em`
  // here would resolve against that 0.8 and shrink it again.
  //
  // Short spans are a `box`: padding on all four sides, and they are short
  // enough never to need a line break. A long one is a `highlight` instead,
  // because a box is a single unbreakable unit — a full path would overhang
  // the margin or stretch the line before it into a row of gaps. The highlight
  // is paint rather than layout, so its side padding has to be real spacing
  // inside the tint (`extent` would paint over the following word space) and
  // its vertical padding comes from the edges, which cost no layout at all.
  // The vertical padding is measured, not guessed: braces, parentheses and
  // descenders reach past the mono font's own line box, and the tint has to
  // stay clear of all of them. The two branches use different mechanisms, so
  // their edges are tuned to land in the same place.
  let code-size = body-size * 0.86
  let code-fill = luma(238)
  let mono-font = ("JetBrains Mono", "Fira Code", "Consolas", "DejaVu Sans Mono")
  let code-tint(body, breakable) = {
    let code = text(size: code-size, body)
    if breakable {
      highlight(
        fill: code-fill,
        extent: 0pt,
        radius: 3pt,
        top-edge: 0.95em,
        bottom-edge: -0.34em,
        { h(0.2em); code; h(0.2em) },
      )
    } else {
      box(fill: code-fill, inset: (x: 0.2em), outset: (y: 0.25em), radius: 3pt, code)
    }
  }
  show raw.where(block: false): it => code-tint(it, it.text.clusters().len() > 40)

  // 8) Code blocks: rounded corners + light grey background + left-gutter line numbers
  // Scope the raw.line rule inside the block rule so it does not fire for
  // inline `code` (which would otherwise get wrapped in a grid and break).
  show raw.where(block: true): it => block(
    fill: luma(245),
    inset: 12pt,
    radius: 6pt,
    width: 100%,
    stroke: none,
    {
      set par(leading: 0.55em, spacing: 0em, first-line-indent: 0pt, justify: false)
      show raw.line: ln => grid(
        columns: (1.6em, 1fr),
        column-gutter: 0.8em,
        align(right + top, text(fill: luma(160), size: 0.9em, str(ln.number))),
        ln.body,
      )
      it
    },
  )
  show raw: set text(font: mono-font)

  // 9) Tables: light grey header + rounded border
  set table(
    stroke: (paint: luma(200), thickness: 0.5pt),
    inset: 8pt,
    fill: (x, y) => if y == 0 { luma(240) } else { none },
  )
  // A column is only as wide as its share of the page, so anything that cannot
  // break inside a cell paints over the neighbouring column. Four defences:
  // trim the padding on wide tables; hyphenate prose; measure every code span
  // against the cell it landed in and give the ones that do not fit the
  // breakable tint (the box branch is one atomic unit — fine in a paragraph,
  // too rigid in a cell); and offer runs the line breaker has no purchase on —
  // identifiers, compounds, hashes — a break between every character. That
  // last character class excludes the punctuation Typst already breaks after,
  // so URLs and paths keep breaking at their natural seams.
  show table: it => block(
    radius: 6pt,
    stroke: 0.5pt + luma(200),
    clip: true,
    inset: 0pt,
    {
      // Past half a dozen columns the padding costs more than the text it
      // frames, so buy the space back sideways.
      let cols = if type(it.columns) == array { it.columns.len() } else { 1 }
      set table(inset: (x: if cols > 6 { 4pt } else { 8pt }, y: 8pt))
      set text(hyphenate: true)
      // The span goes in as plain mono text, not as `raw`: the rule above also
      // matches `raw`, and a nested match would tint the span twice.
      show table.cell: c => layout(cell => {
        show raw.where(block: false): it => context {
          let code = text(font: mono-font, it.text)
          code-tint(code, measure(code-tint(code, false)).width > cell.width)
        }
        c
      })
      show regex("[^\\s\u{200b}/?&=#,;:]{15,}"): t => t.text.clusters().join("\u{200b}")
      it
    },
  )
  show table: set par(justify: false, spacing: 0.6em)
  show table.cell.where(y: 0): set text(weight: "bold")

  // 10) Highlight: a softer yellow
  show highlight: set highlight(fill: rgb(tokens.base.at("mark-bg")))

  // Cover page — the first content this template emits, so it lands on page 1.
  // `page()` terminates itself; a `pagebreak()` after it would add a blank page.
  if cover-mode {
    cover-page(
      title: title,
      subtitle: cover-subtitle,
      authors: authors,
      date: cover-date,
      logo: args.at("cover-logo", default: none),
      palette: args.at("cover-color", default: "ocean"),
      decoration: if type(cover-style) == str { cover-style } else { "arcs" },
      cover-image: cover-image,
      text-color: args.at("cover-text-color", default: black),
      remotes: remotes,
      asset: asset,
    )
  }

  // Letter mode (DIN 5008 Form B). Coordinates are page-absolute so the
  // address lines up with the window of a DIN long / C6/5 envelope. Typst's
  // `place()` is column-relative, so we subtract the page margins.
  //
  // DIN 5008 Anschriftfeld (80mm × 45mm) at 25mm from page left, 45mm from
  // page top, composed of two zones:
  //   - Zusatz- und Vermerkzone: 80mm × 17.7mm — return line lives here
  //   - Anschriftzone:           80mm × 27.3mm — recipient address
  // Each zone is a fixed-height block so the recipient address always starts
  // exactly at 45 + 17.7 = 62.7mm from the page top, regardless of how many
  // lines the return zone contains.
  if letter-mode {
    place(top + left,
      dx: 25mm - page-margin-x,
      dy: 45mm - page-margin-y,
      block(width: 80mm, height: 45mm, {
        // Stack the two zones with zero inter-block spacing so the total
        // height stays exactly 17.7 + 27.3 = 45mm. Without this, Typst's
        // default block spacing (~1.2em) would push the Anschriftzone down
        // and the recipient would no longer start at 62.7mm from page top.
        set block(spacing: 0pt)
        // Zusatz- und Vermerkzone (17.7mm) — return line, small + underlined.
        // Place near the bottom but with a small gap above the recipient so
        // the underline doesn't visually touch the recipient's first line.
        block(width: 80mm, height: 17.7mm, {
          if letter-return != "" {
            place(bottom + left, dy: -2mm, text(size: 8pt, underline(letter-return)))
          }
        })
        // Anschriftzone (27.3mm) — up to 6 lines of recipient address.
        block(width: 80mm, height: 27.3mm, {
          set par(leading: 0.5em, spacing: 0.3em)
          for line in letter-to {
            text(size: 11pt, line)
            linebreak()
          }
        })
      }))

    // Infofeld (sender details) on the right. Vertically aligned with the
    // Anschriftzone (recipient block) so both blocks share the same top
    // baseline at 45 + 17.7 = 62.7mm from the page top.
    if letter-from.len() > 0 {
      place(top + left,
        dx: 125mm - page-margin-x,
        dy: 62.7mm - page-margin-y,
        block(width: 75mm, {
          set par(leading: 0.5em, spacing: 0.3em)
          for line in letter-from {
            text(size: 10pt, line)
            linebreak()
          }
        }))
    }

    // Reserve vertical space so normal flow starts at the DIN 5008 subject
    // position (98.46mm from the page top). The cursor is currently at the
    // top of the content area (page-margin-y from the page top), so advance
    // 98.46mm - page-margin-y.
    v(98.46mm - page-margin-y)

    if letter-subject != "" or letter-date != "" {
      // Subject left, place + date right, same baseline.
      grid(
        columns: (1fr, auto),
        column-gutter: 1em,
        text(weight: "bold", size: 11pt, letter-subject),
        text(size: 11pt, letter-date),
      )
      v(1.5em)
    }
  }

  // Title area (optional) — the cover already carries the title when present.
  if title != "" and not cover-mode {
    let byline = authors + if date == none or date == "" { () } else { (date,) }
    align(center)[
      #text(1.8em, weight: "black", title)
      #if byline.len() > 0 [
        #v(0.35em)
        #text(0.95em, fill: rgb("#555555"), byline.join(" · "))
      ]
    ]
    v(1em)
  }

  body
}
