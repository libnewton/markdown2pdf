# md2pdf design

## What it is

Markdown goes in. A typeset PDF or a self-contained HTML page comes out.
Everything runs where the document already is: the browser tab, or the machine
that holds the file. There is no account, no upload and no server.

The design serves one requirement: **one document, two outputs, no
divergence**. A `.md` file rendered to PDF and to HTML must say the same
thing: the same headings, tables, callouts and footnote numbers. That holds
even though Typst sets one and someone else's browser styles the other.

## The shape that follows

One parser and two renderers, in the same Rust crate:

```
              ┌──────────────────────────────┐
  .md ───────▶│  engine/  (comrak → AST)     │
              │   typst.rs ──▶ Typst markup  │──▶ Typst ──▶ PDF / page SVG
              │   html/    ──▶ HTML          │──▶ a page, or a download
              └──────────────────────────────┘
```

The preprocessing (callouts, spoilers, `+`-width tables, lettered lists,
HedgeDoc and Outline syntax, citations, blank-line handling) runs once, before
either renderer. The frontmatter is parsed once too, in the engine, and Typst
reads the engine's JSON instead of calling `yaml()`. Neither renderer can
quietly disagree about what the document is. Where the outputs must differ
(Typst needs `#link(…)`, HTML needs `<a href>`), they differ only at the last
step.

This property has failed before. A `title:` in the frontmatter once made the
HTML renderer drop the leading `# H1` while the PDF kept it. Each side was
tested on its own, and nothing compared them. `engine/src/html/tests.rs` now
runs a cross-renderer parity suite and a "nothing disappears" sweep over every
fixture in `tests/`, because divergence is the failure this architecture is
most exposed to.

## Why Typst, and why in the browser

Typst compiles to WebAssembly, which is what makes a PDF pipeline without a
server possible. The alternatives, LaTeX or a headless browser's print path,
need a server. A server needs uploads, and then the document leaves the
machine.

The cost is download size. To work offline the app caches about 40 MB, and
the Typst compiler alone is about 30 MB. The service worker caches it once per
version. The HTML preview does not wait for any of it, because it needs only
the engine.

## Two previews, deliberately different

- **Pages** is Typst's own output, page by page, as SVG. It is a proof of the
  printed result: what you see is what the PDF has. Its text is drawn glyphs,
  so nothing in it is interactive.
- **Web** is the HTML renderer's output, mounted in a shadow root. It has no
  pages and updates instantly. Here the document behaves like a document:
  outline navigation, copyable code blocks, checkboxes you can tick.

Neither is a lesser version of the other. The split is why the Web view can
update on every keystroke while the Pages view compiles only when it is on
screen. (It once ran a full Typst compile per keystroke for a pane nobody was
looking at.)

## The one place the view writes back

Ticking a checkbox in the Web preview edits the source. That is the only
exception to "the view does not change the document", and its scope is narrow:

- The engine emits `data-md-line` on the item, carrying the line **the author
  wrote**. The preprocess passes move lines around, so the origin is carried
  through them rather than reconstructed afterwards.
- The app owns the write, not the engine's script. That script ships inside
  every downloaded HTML file, where there is no source to write to.
- A click on a stale render does nothing rather than corrupt the source. The
  write is refused unless that line still holds a task marker.
- Pages, the PDF and the export keep static checkboxes. A printed checkbox
  cannot be clicked, and pretending otherwise would mislead.

Editor/preview scroll sync was built on the same line origins and then
removed. Following the scroll position in the other pane got in the way more
often than it helped. The origins stay because the checkbox write needs them,
and a better attempt at sync would not have to rebuild them.

## Trust boundaries

A `.md` file is untrusted input: pasted, dropped, or opened from a link
someone sent. Two places can be harmed. One is the exported HTML file, opened
by whoever receives it. The other is the app, where running script reaches
every document in IndexedDB.

There is no CSP. That is a deliberate choice, and it means escaping and
allowlists are the control itself, not a second line of defence:

- Every string that reaches markup goes through `esc_text` or `esc_attr`, and
  every attribute is quoted. The tests check the structure of the output (no
  tag or attribute outside a known set, no `on*`, no scheme outside
  `http(s)`/`data:image/`) over every fixture, not a hand-picked few.
- Link schemes are allowlisted in **both** renderers. They used to be checked
  only on the HTML side, which let a `javascript:` URL reach Typst's link
  annotation and from there the SVG preview, which lives in the live document,
  not a shadow root.
- Third-party markup is not inlined. Mermaid's SVG is embedded as an `<img>`
  data URI, where it can neither run script nor fetch. `math-core`'s MathML is
  the one exception, and the same tag and attribute guards apply to it.
- Nothing is hoisted out of the shadow root. The preview handles code copying
  and anchor scrolling itself. The engine emits a script only for the
  standalone export, where there is no shadow root to escape.
- Raw HTML is text, except a short list of inline formatting tags (`<b>`,
  `<kbd>`, `<sup>`, `<br>` and a few more). Those render as formatting with
  every attribute dropped, so they cannot carry a handler or a URL. HTML
  comments are dropped.

## Three front-ends, one engine

| | Markdown | Typesetting | Notes |
|---|---|---|---|
| Web app | `engine.wasm` | Typst in WebAssembly | Offline after first load |
| CLI (`bin/md2pdf`) | `engine.wasm` via the Typst package | system `typst` | Python stdlib only |
| HTML export | `engine.wasm` | none | One file, no external requests |

The CLI is a thin host, not a second implementation. It fetches the remote
images Typst cannot fetch itself, then hands the document to `typst`.
Everything about what Markdown means lives in `engine/`, so a fix reaches all
three at once.

Fonts are the one thing that cannot be shared. The web app fetches a CJK face
on demand, the CLI uses installed system fonts, and the HTML export asks for
the reader's system fonts. These are three answers to one problem, because the
three environments get fonts from different places.

The welcome document in the web app is `tests/extended.md` itself, imported
at build time. The app's feature demo and the CLI's render fixture are the same
file, so they cannot drift apart.

## What is knowingly not solved

- **The HTML export does not look like the PDF.** The markup matches, the
  type does not. The PDF sets IBM Plex Sans, the export asks for the reader's
  system fonts. Closing the gap means embedding a subsetted face in each file.
  That is worth an opt-in, not a default.
- **Twemoji is 3689 files and 17 MB** copied into the deploy, fetched one
  glyph at a time at runtime. It is left out of the precache but still part of
  the build.

Two items from earlier versions of this list are solved. CI now rebuilds
`package/engine.wasm` and fails if the committed file does not match the
source. `bin/md2pdf` now has 48 tests in `tests/test_cli.py`.
