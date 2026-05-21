// Modern Tech style.
// Sans-serif throughout (web-like reading), paragraph spacing, no first-line indent, modern code blocks.

#import "../admonitions.typ": admonition, spoiler, task-item, md2pdf-list-markers, md2pdf-enum-numbering

#let article(title: "", authors: (), ..args, body) = {
  let page-numbers = args.at("page-numbers", default: true)
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
  // 1) Page setup: wide margins for readability
  // Letter mode uses 20mm x-margin (DIN 5008 body margin).
  let page-margin-x = if letter-mode { 20mm } else { 1.8cm }
  let page-margin-y = 2cm
  set page(
    paper: "a4",
    margin: (x: page-margin-x, y: page-margin-y),
    numbering: if page-numbers { "1" } else { none },
    // Only show the page-number footer when there are 2+ pages.
    // A single-page document doesn't need "1" at the bottom.
    footer: if page-numbers {
      context {
        let total = counter(page).final().first()
        if total > 1 {
          align(center, text(9pt, fill: luma(120), counter(page).display("1")))
        }
      }
    },
  )
  set document(title: title, author: authors, date: none)

  // 2) Font stack: high-quality Latin sans-serif fonts for text/numbers
  set text(
    font: (
      "IBM Plex Sans",
      "Roboto",
      "Libertinus Sans",
    ),
    size: 10.5pt,
    lang: "en",
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

  // 4) Headings: bold, dark grey, generous spacing (clear hierarchy)
  show heading: it => {
    set text(
      weight: "bold",
      fill: rgb("#333333"),
      font: ("IBM Plex Sans", "Roboto"),
    )
    block(above: 2em, below: 1em, it)
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

  // 11) Images: centered (the generator adds a caption when alt text is present)
  show image: it => align(center, it)

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

  // Title area (optional)
  if title != "" {
    align(center)[
      #text(1.8em, weight: "black", title)
      #if authors.len() > 0 [
        #v(0.35em)
        #text(0.95em, fill: rgb("#555555"), authors.join(", "))
      ]
    ]
    v(1em)
  }

  body
}
