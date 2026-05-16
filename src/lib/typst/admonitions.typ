// Shared callout / spoiler / task-item helpers.
// Imported by every style template.

#let _admonition-themes = (
  "success": (accent: rgb("#16A34A"), bg: rgb("#F0FDF4"), label: "SUCCESS"),
  "warning": (accent: rgb("#D97706"), bg: rgb("#FFFBEB"), label: "WARNING"),
  "tip":     (accent: rgb("#0EA5E9"), bg: rgb("#F0F9FF"), label: "TIP"),
  "info":    (accent: rgb("#2563EB"), bg: rgb("#EFF6FF"), label: "INFO"),
  "danger":  (accent: rgb("#DC2626"), bg: rgb("#FEF2F2"), label: "DANGER"),
  "note":    (accent: rgb("#6B7280"), bg: rgb("#F9FAFB"), label: "NOTE"),
)

#let admonition(kind: "info", title: "", body) = {
  let theme = _admonition-themes.at(kind, default: _admonition-themes.at("info"))
  let label = if title != "" { title } else { theme.at("label") }
  block(
    fill: theme.at("bg"),
    stroke: (left: 3pt + theme.at("accent")),
    inset: (left: 12pt, right: 12pt, top: 10pt, bottom: 10pt),
    radius: 6pt,
    width: 100%,
    {
      text(weight: "bold", fill: theme.at("accent"), size: 0.9em, label)
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
      stroke: 1pt + rgb("#16A34A"),
      fill: rgb("#16A34A"),
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
  block(
    width: 100%,
    above: 0.35em,
    below: 0.35em,
    {
      stack(dir: ltr, spacing: 0.5em, mark, body)
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
