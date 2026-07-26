// Modern Tech style.
// Sans-serif throughout (web-like reading), paragraph spacing, no first-line indent, modern code blocks.

#import "../admonitions.typ": admonition, spoiler, task-item, md2pdf-list-markers, md2pdf-enum-numbering
#import "../furniture.typ": bar
#import "../cover.typ": cover-page

#let article(title: "", authors: (), ..args, body) = {
  let page-numbers = args.at("page-numbers", default: true)
  let date = args.at("date", default: none)
  let lang = args.at("lang", default: "en")
  let region = args.at("region", default: none)
  let remotes = args.at("remotes", default: ())
  let asset = args.at("asset", default: image)
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
  let footer-slots = (
    slot-of("footer-left"),
    slot-of("footer-center", fallback: number-template),
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
  set page(
    paper: "a4",
    margin: (x: page-margin-x, y: page-margin-y),
    numbering: if page-numbers != false { "1" } else { none },
    header-ascent: 6mm,
    footer-descent: 6mm,
    // Page furniture starts on the second page: page one is either the cover
    // or the title page, and neither wants a running head.
    header: context {
      if here().page() > 1 { bar(..header-slots, vars, remotes, asset) }
    },
    footer: context {
      if here().page() > 1 { bar(..footer-slots, vars, remotes, asset) }
    },
  )
  set document(title: title, author: authors, date: none)

  // 2) Font stack: high-quality Latin sans-serif fonts for text/numbers
  let body-size = 10.5pt
  set text(
    font: (
      "IBM Plex Sans",
      "Roboto",
      "Libertinus Sans",
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
        fill: rgb("#333333"),
        font: ("IBM Plex Sans", "Roboto"),
        size: body-size * style.size,
        tracking: style.tracking,
        marker + it.body,
      ),
    )
  }

  // 5) Link colour: tech blue
  show link: set text(fill: rgb("#0074de"))

  // 6) Blockquotes: left accent line + light background
  set quote(block: true)
  show quote: it => {
    set par(first-line-indent: 0pt)
    block(
      fill: luma(248),
      stroke: (left: 2pt + rgb("#0074de")),
      inset: (left: 0.9em, right: 0.9em, top: 0.7em, bottom: 0.7em),
      radius: 6pt,
      width: 100%,
      it.body,
    )
  }

  // 7) Inline code: light background + rounded corners
  // Inline code sits at 0.95em — close enough to the body to read at a
  // glance, but a hair smaller because monospace x-height runs hot.
  // `outset` extends the background up/down beyond the layout box so tall
  // glyphs (brackets, descenders) sit inside the tint without pushing the
  // surrounding line apart.
  show raw.where(block: false): it => box(
    fill: luma(238),
    inset: (x: 4pt, y: 0pt),
    outset: (top: 2pt, bottom: 3pt),
    radius: 3pt,
    text(size: 0.95em, it),
  )

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
  show raw: set text(font: ("JetBrains Mono", "Fira Code", "Consolas", "DejaVu Sans Mono"))

  // 9) Tables: light grey header + rounded border
  set table(
    stroke: (paint: luma(200), thickness: 0.5pt),
    inset: 8pt,
    fill: (x, y) => if y == 0 { luma(240) } else { none },
  )
  show table: it => block(
    radius: 6pt,
    stroke: 0.5pt + luma(200),
    clip: true,
    inset: 0pt,
    it,
  )
  show table: set par(justify: false, spacing: 0.6em)
  show table.cell.where(y: 0): set text(weight: "bold")

  // 10) Highlight: a softer yellow
  show highlight: set highlight(fill: rgb("#FEF08A"))

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
    align(center)[
      #text(1.8em, weight: "black", title)
      #if authors.len() > 0 [
        #v(0.35em)
        #text(0.95em, fill: rgb("#555555"), authors.join(", "))
      ]
    ]
    v(1em)
  }

  {
    // Images: centered (the generator adds a caption when alt text is present).
    // Scoped to the body so it cannot stretch the cover logo or a header image,
    // which are sized boxes rather than full-width figures.
    show image: it => align(center, it)
    body
  }
}
