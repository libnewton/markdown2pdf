// Optional title cover page.
//
// Two independent choices: a geometry preset (`cover:`) drawn into the page
// background, and a colour palette (`cover-color:`). Everything is anchored to
// the physical page edges — background content resolves `100%` against the full
// page, ignoring margins.

#import "furniture.typ": image-args, parse-image-field, resolve-path

// Palettes tint the decoration and the hairline only — cover text stays black.
#let cover-palettes = (
  ocean: (primary: rgb("#0C4A6E"), secondary: rgb("#0284C7"), tint: rgb("#BAE6FD")),
  ink: (primary: rgb("#0F172A"), secondary: rgb("#475569"), tint: rgb("#CBD5E1")),
  ember: (primary: rgb("#7C2D12"), secondary: rgb("#EA580C"), tint: rgb("#FED7AA")),
  plum: (primary: rgb("#4C1D95"), secondary: rgb("#7C3AED"), tint: rgb("#DDD6FE")),
)

// A palette name, or any hex colour — tints are derived from it.
#let resolve-palette(name) = {
  if type(name) != str or name == "" { return cover-palettes.ocean }
  if name in cover-palettes { return cover-palettes.at(name) }
  if not name.starts-with("#") { return cover-palettes.ocean }
  let c = rgb(name)
  (primary: c, secondary: c.lighten(25%), tint: c.lighten(75%))
}

// The template fixes `paper: "a4"`, so page-edge geometry can be absolute.
#let _w = 210mm

// Concentric quarter-discs at the bottom-left corner. `circle` is centred on
// the corner (dx: -r, dy: +r); the off-page three quarters simply is not drawn.
#let _arcs(pal) = {
  for (r, fill) in ((108mm, pal.tint), (80mm, pal.secondary), (54mm, pal.primary)) {
    place(bottom + left, dx: -r, dy: r, circle(radius: r, fill: fill, stroke: none))
  }
}

// Slanted bands across the bottom third. All polygon vertices stay >= 0 — the
// shape's bounding box starts at its minimum vertex, so a negative coordinate
// would silently shift the whole band.
#let _strata(pal) = {
  let bands = ((88mm, 26mm, pal.tint), (66mm, 20mm, pal.secondary), (42mm, 16mm, pal.primary))
  for (base, rise, fill) in bands {
    place(
      bottom + left,
      polygon(fill: fill, stroke: none, (0mm, rise), (_w, 0mm), (_w, base + rise), (0mm, base + rise)),
    )
  }
  place(bottom + left, rect(width: _w, height: 1.5mm, fill: pal.secondary))
}

// One diagonal wedge filling the bottom-right, with a thin echo of its
// hypotenuse offset above it.
#let _wedge(pal) = {
  place(bottom + left, dy: -6mm, line(
    start: (0mm, 104mm),
    end: (_w, 0mm),
    stroke: 0.8pt + pal.secondary,
  ))
  place(bottom + left, polygon(fill: pal.primary, stroke: none, (0mm, 104mm), (_w, 0mm), (_w, 104mm)))
}

// Dot lattice fading upward over two solid bars. A `tiling` cannot do this —
// every tile is identical, so the per-row opacity ramp has to be drawn out.
#let _grid(pal) = {
  let (pitch, rows, cols) = (7mm, 14, 31)
  for row in range(rows) {
    let opacity = 60% * (row + 1) / rows
    for col in range(cols) {
      place(
        bottom + left,
        dx: 3mm + col * pitch,
        dy: -16mm - (rows - 1 - row) * pitch,
        circle(radius: 0.6mm, fill: pal.secondary.transparentize(100% - opacity), stroke: none),
      )
    }
  }
  place(bottom + left, dy: -10mm, rect(width: _w, height: 3mm, fill: pal.tint))
  place(bottom + left, rect(width: _w, height: 10mm, fill: pal.primary))
}

#let cover-decorations = (arcs: _arcs, strata: _strata, wedge: _wedge, grid: _grid)

#let resolve-decoration(name) = {
  if type(name) == str and name in cover-decorations { cover-decorations.at(name) } else { _arcs }
}

// The cover itself: one self-terminating `page()`. It inherits the surrounding
// `set page` for anything not overridden, and does not touch the page counter —
// so the cover is page 1 and the first content page is 2.
#let cover-page(
  title: "",
  subtitle: "",
  authors: (),
  date: "",
  logo: none,
  palette: "ocean",
  decoration: "arcs",
  remotes: (),
  asset: image,
  margin-x: 24mm,
) = {
  let pal = resolve-palette(palette)
  page(
    header: none,
    footer: none,
    numbering: none,
    margin: (x: margin-x, y: 22mm),
    background: resolve-decoration(decoration)(pal),
    {
      if logo != none and logo != "" {
        let img = parse-image-field(logo)
        if img != none {
          place(top + right, asset(resolve-path(img.path, remotes), ..image-args(img, 16mm)))
        }
      }

      // Title block starts ~100mm down the page (content area begins at 22mm).
      // All cover text is black; only the hairline carries the palette colour.
      v(78mm)
      set par(justify: false, leading: 0.5em, spacing: 0pt)
      set text(fill: black)
      text(30pt, weight: "black", title)
      if subtitle != "" {
        v(4mm)
        text(15pt, subtitle)
      }
      v(12mm)
      line(length: 20mm, stroke: 1.5pt + pal.secondary)
      v(5mm)
      if authors.len() > 0 {
        text(11pt, authors.join(", "))
        linebreak()
      }
      if date != "" { text(11pt, date) }
    },
  )
}
