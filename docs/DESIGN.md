# md2pdf — Design

## What it is

Markdown in, a typeset PDF or a self-contained HTML page out. Everything runs
where the document already is: the browser tab, or the machine holding the
file. No account, no upload, no server.

The claim the whole design serves is **one document, two artefacts, no
divergence**. A `.md` file rendered to PDF and the same file rendered to HTML
must say the same thing — same headings, same tables, same admonitions, same
footnote numbering — even though one is set by Typst and the other is a
stylesheet in someone else's browser.

## The shape that follows from it

One parser, two renderers, both in the same Rust crate:

```
              ┌──────────────────────────────┐
  .md ───────▶│  engine/  (comrak → AST)     │
              │    lib.rs   ──▶ Typst markup │──▶ Typst ──▶ PDF / page SVG
              │    html/    ──▶ HTML         │──▶ a page, or a download
              └──────────────────────────────┘
```

Not "an HTML renderer and, separately, a PDF renderer". The preprocessing —
admonitions, spoilers, `+`-width tables, citations, blank-line normalisation —
happens once, before either renderer runs, so neither can quietly disagree
about what the document *is*. Where they must differ (Typst needs `#link(…)`,
HTML needs `<a href>`) they differ at the last step only.

That property is load-bearing, and it is the one that has actually failed:
`title:` in frontmatter used to make the HTML renderer drop the leading `# H1`
while the PDF kept it. Both sides were individually tested; nothing compared
them. `engine/src/html/tests.rs` now runs a cross-renderer parity suite and a
"nothing silently disappears" sweep over every fixture in `tests/`, because a
divergence is the failure mode this architecture is uniquely exposed to.

## Why Typst, and why in the browser

Typst compiles to WASM, which is what makes a client-only PDF pipeline
possible at all. The alternative — LaTeX, or a headless browser's print path —
means a server, which means uploads, which means the document leaves the
machine. It does not.

The cost is paid in bytes: the compiler, the engine and the fonts are a few MB
that have to arrive before the first PDF. They are cached by the service
worker, so that is a once-per-version cost, and the HTML preview does not wait
for any of it — it needs the engine only.

## Two previews, deliberately different

- **Pages** is Typst's own output, page by page, as SVG. It is a proof of the
  printed artefact: what you see is what the PDF has. Text in it is drawn
  glyphs, so there is no element identity and nothing in it is interactive.
- **Web** is the HTML renderer's output, mounted in a shadow root. Pageless,
  instant, and the mode in which the document is a *document* rather than a
  proof — outline navigation, copyable code blocks, checkboxes you can tick.

Neither is a lesser version of the other. The split is why the Web view can
update on every keystroke while the paged view compiles only when it is on
screen — which it now does, having previously run a full Typst compile per
keystroke for a pane nobody was looking at.

## The one place the view writes back

Ticking a checkbox in the Web preview edits the source. That is the single
exception to "the view does not change the document", and it is scoped to be
honest about it:

- The engine emits `data-md-line` on the item, carrying the line **the author
  wrote** — the preprocess passes shift lines around, so the origin is
  threaded through them rather than reconstructed afterwards.
- The app, not the engine's script, owns the write. The engine's script ships
  inside every downloaded HTML file, where there is no source to write to.
- A click against a stale render is a no-op, not a corruption: the write is
  refused unless that line still carries a task marker in the state being
  asked about.
- Pages, the PDF and the export keep inert checkboxes. A printed checkbox is
  not clickable, and pretending otherwise would be the lie.

Editor↔preview scroll sync was built on the same line origins and then
removed: following the reader's scroll in the other pane fought them more
often than it helped. The origins stay because the checkbox write needs them,
and because a better attempt at sync would not have to rebuild them.

## Trust boundaries

A `.md` file is untrusted input — pasted, dropped, or opened from a link
someone sent. Two blast radii matter: the exported HTML file, opened by
whoever received it, and the app itself, where script execution reaches every
document in IndexedDB.

There is no CSP. That is a deliberate call, and it means the escaping and the
allowlists *are* the control rather than a second line of defence:

- Every string reaching markup goes through `esc_text` or `esc_attr`; every
  attribute is quoted. The test suite asserts structurally — no tag outside a
  known set, no attribute outside a known set, no `on*`, no scheme outside
  `http(s)`/`data:image/` — over every fixture, not over a hand-picked few.
- Link schemes are allowlisted in **both** renderers. They used to be checked
  on the HTML side only, which let a `javascript:` URL reach Typst's link
  annotation and, from there, the SVG preview — which lives in the live
  document, not a shadow root.
- Third-party markup is not inlined. Mermaid's SVG is embedded as an `<img>`
  data URI, where it can neither script nor fetch. `math-core`'s MathML is the
  one exception left, and it is held to the same tag/attribute guards.
- Nothing is hoisted out of the shadow root. The preview implements code
  copying and anchor scrolling itself; the engine emits a script only for the
  standalone export, where there is no shadow root to escape.
- `<br>` is the only raw HTML tag either renderer honours — the one way to
  break a line inside a table cell. Everything else is text.

## Three front-ends, one engine

| | Markdown | Typesetting | Notes |
|---|---|---|---|
| Web app | `engine.wasm` | Typst-in-WASM | Offline after first load |
| CLI (`bin/md2pdf`) | `engine.wasm` via the Typst package | system `typst` | Python stdlib only |
| HTML export | `engine.wasm` | none | One file, no external requests |

The CLI is a shim, not a second implementation: it resolves the remote images
Typst cannot fetch itself, then hands the document to `typst`. Everything about
what Markdown *means* lives in `engine/`, so a fix lands in all three at once.

Type is the one thing that cannot be shared. The web app fetches a CJK face on
demand, the CLI consults installed system fonts, and the HTML export asks for
the reader's system stack — three answers to the same problem, because the
three environments have nothing in common about where a font comes from.

## What is knowingly not solved

- **The HTML export does not look like the PDF.** The markup matches; the type
  does not. The PDF sets IBM Plex Sans, the export asks for the reader's system
  stack. Closing it means embedding a subsetted face per file — worth an
  opt-in, not a default.
- **`package/engine.wasm` is committed and nothing checks it is current.** A
  change to `engine/` without `./build.sh` ships a stale engine to the web app
  while `cargo test` stays green.
- **Twemoji is 3689 files and 17 MB** copied into the deploy artefact, fetched
  one glyph at a time at runtime. Excluded from the precache; still in the
  build.
- **`bin/md2pdf` has no tests.** It is the whole CLI.
