// Shared callout / spoiler / task-item helpers.
// Imported by every style template.

#import "tokens.typ": tokens

#let admonition(kind: "info", title: "", lang: "en", body) = {
  let themes = tokens.admonition
  let theme = themes.at(kind, default: themes.at("info"))
  let language = if lang == "de" { "de" } else { "en" }
  let label = if title != "" { title } else { theme.at(language) }
  block(
    fill: rgb(theme.bg),
    stroke: (left: 3pt + rgb(theme.accent)),
    inset: (left: 12pt, right: 12pt, top: 10pt, bottom: 10pt),
    radius: 6pt,
    width: 100%,
    {
      text(weight: "bold", fill: rgb(theme.accent), size: 0.9em, label)
      v(0.7em, weak: true)
      body
    },
  )
}

#let spoiler(summary: "spoiler", body) = {
  // Build the header content once so we can measure it for body alignment.
  let header = text(weight: "bold", {
    sym.triangle.filled.r
    h(0.4em)
  })
  block(
    fill: luma(248),
    stroke: 0.5pt + luma(220),
    inset: (left: 10pt, right: 10pt, top: 8pt, bottom: 8pt),
    radius: 6pt,
    width: 100%,
    {
      text(weight: "bold", {
        sym.triangle.filled.r
        h(0.4em)
        summary
      })
      v(0.7em, weak: true)
      // Indent the body to align with where `summary` starts (right of the ▶ + gap).
      context {
        let m = measure(header)
        pad(left: m.width, body)
      }
    },
  )
}

#let task-item(checked, body) = {
  let mark = if checked {
    box(
      width: 0.95em,
      height: 0.95em,
      stroke: 1pt + rgb(tokens.base.ok),
      fill: rgb(tokens.base.ok),
      radius: 2pt,
      align(center + horizon, text(white, size: 0.8em, weight: "bold", [✓])),
    )
  } else {
    box(
      width: 0.95em,
      height: 0.95em,
      stroke: 1pt + luma(140),
      radius: 2pt,
    )
  }
  // Items need more air between them than the 1em leading inside one, or a
  // checklist of wrapped items reads as a single block of text.
  block(
    width: 100%,
    above: 1em,
    below: 1em,
    {
      // The stack lines the box up with the top of the first *line box*, which
      // sits above the cap height — hence the nudge, so the box reads as
      // centred on the first line instead of hanging below its baseline.
      stack(dir: ltr, spacing: 0.5em, move(dy: -0.1em, mark), body)
    },
  )
}

// Multi-depth list markers (1st: filled circle, 2nd: filled square, 3rd+: hollow circle)
#let md2pdf-list-markers = ([•], [▪], [◦])

// Multi-depth enum numbering: 1. → a) → i)
#let md2pdf-enum-numbering(..nums) = {
  let n = nums.pos()
  if n.len() >= 3 {
    numbering("i)", n.last())
  } else if n.len() == 2 {
    numbering("a)", n.last())
  } else {
    numbering("1.", n.last())
  }
}
