# md2pdf — Engineering

Working notes for the code as it is. `docs/DESIGN.md` covers why it is shaped
this way.

## Hard constraints

- **No server.** SvelteKit with `@sveltejs/adapter-static`, everything
  prerendered. No `+server.ts`, no form actions, no runtime host.
- **Prerender boundary.** Route modules run at build time. `window`,
  `document`, `indexedDB`, `caches` are reachable only inside `onMount()` or
  behind `if (browser)`.
- **Offline after first load.** Fonts, the Typst package, the engine and the
  Twemoji mirror are all same-origin. The only outbound requests at runtime
  are image URLs the document itself asked for.
- **One Markdown implementation.** If a change to what Markdown *means* does
  not live in `engine/`, it is in the wrong place.

## Layout

```
engine/                     Rust, one crate, two renderers
  src/lib.rs                Typst markup + every preprocess pass + the ABI
  src/html/mod.rs           HTML renderer
  src/html/highlight.rs     the syntax highlighter
  src/html/css.rs           the document stylesheet and the export's script
  src/html/tokens.rs        colour tokens, shared with the Typst side
  src/html/assets.rs        the key<TAB>len wire format for asset blobs
  src/html/math.rs          math-core bridge
  src/html/tests.rs         141 tests, incl. the cross-renderer parity suite

package/                    the `md2pdf` Typst package
  lib.typ                   prepare() for PDF, prepare-html() for the CLI
  styles/modern-tech.typ    the one document style
  engine.wasm               committed; rebuilt by ./build.sh
  vendor/                   mitex (math), mmdr (mermaid)
  twemoji/                  3689 SVGs

bin/md2pdf                  the CLI and local stdio MCP server; Python stdlib only
tests/                      Markdown fixtures + check_html.py

web/src/
  routes/                   /  and  /reference  — that is all of them
  lib/components/
    PdfEditor.svelte        state, wiring, layout, shortcuts, export
    EditorPane.svelte       wraps the editor, owns image paste/drop
    MarkdownEditor.svelte   CodeMirror 6
    HtmlPreview.svelte      the shadow-root mount for the Web view
    DocumentMenu.svelte     recent documents and templates
    ShortcutOverlay.svelte  the cheatsheet
    StatusHint.svelte       the floating "Updating preview" pill
  lib/workers/
    typst.worker.ts         Typst compiler + the engine, both in the worker
    typstClient.ts          the main-thread wrapper
    compileProtocol.ts      the message types, shared by both sides
    assetBundle.ts          decodes the engine's asset wire format
  lib/editor/commands.ts    slash commands and the formatting keymap
  lib/typst/                vector IR → per-page SVG, and the SVG scrubber
  lib/storage/documents.ts  IndexedDB
  lib/stores/               document + settings state (Svelte 5 runes)
  lib/utils/                remote images, image helpers, task-marker parsing
```

## Commands

```sh
cd engine && cargo test        # the engine's 141 tests
./build.sh                     # rebuild package/engine.wasm, install @local/md2pdf
cd web && npm install
npm run check                  # svelte-check; must be 0 errors 0 warnings
npm test                       # vitest, node environment
npm run test:e2e               # playwright, real browser
npm run format:check           # prettier
npm run dev
npm run build                  # → web/build/
python3 bin/md2pdf tests/sample.md
python3 tests/check_html.py out.html
```

`./build.sh` needs `rustup target add wasm32-unknown-unknown`. **Any commit
touching `engine/` must re-run it and commit `package/engine.wasm`** — the web
build has no Rust toolchain and reads the committed artefact.

## The engine ABI

`wasm-minimal-protocol`; the Typst package calls these through `plugin()`, and
the worker calls the same module through a hand-written host in
`typst.worker.ts`.

| Function | Returns |
|---|---|
| `convert(md, strip_h1, citations)` | Typst markup |
| `render_html(md, options)` | `warnings` `\u{1F}` `html` |
| `html_assets(md)` | images, remotes, twemoji and mermaid keys in one pass |
| `leading_h1(md)` | the first H1's text, for the title rule |
| `remotes(md)` / `twemojis(md)` / `html_images(md)` / `html_fonts(md)` / `html_mermaid(md)` | single-purpose lists, for `lib.typ` |
| `tokens()` | the colour palette, so Typst and HTML agree |
| `inline_bibliography(md)` / `without_inline_bibliography(md)` | the citation split |

`html_assets` exists because the HTML path used to make five separate calls,
each re-parsing the whole document. It is one parse; the worker uses it and the
single-purpose functions remain for `lib.typ`.

`render_html` options are a `key=value` list: `standalone` wraps the fragment
in a full document (the download), `editable` adds `data-md-line` and live
checkboxes (the preview only — never a download), and `theme` fixes the initial
standalone colour scheme to `light` or `dark` (`system` leaves it automatic).

`md2pdf mcp --root PATH` is a newline-delimited JSON-RPC stdio server. It keeps
the wire isolated by running normal renders as captured child CLI processes,
restricts file paths to the configured root, and exposes the canonical syntax
and example documents as fixed resources.

## Line origins

The preprocess passes are not line-preserving: blank runs collapse to three
lines, admonitions and spoilers collapse a whole block and re-parse the body as
a fresh document, `+`-width tables insert a line. So the passes carry
`(String, u32)` — text plus the 1-based line it came from, `0` for lines a pass
invented — and `render_source` rebases its own origins into original-document
coordinates once, up front.

This is what makes `data-md-line` mean "the line the author wrote" rather than
"the line after preprocessing", and it is what the task-checkbox write stands
on. **The Typst renderer ignores these fields entirely.**

The engine still tags every top-level block, not only task items. That fed
editor↔preview scroll sync, which has been removed; the attribute costs a few
bytes in the preview, never appears in a download, and keeps the door open if
sync is attempted again.

## The worker

```ts
// request
{ type: 'compile', id, markdown, images, pageNumbers, format: 'pdf' | 'preview' }
{ type: 'html',    id, markdown, images, standalone?, editable? }
// response
{ type: 'compile-result', id, ok, pdf? | preview? | html?, diagnostics, error? }
```

- Typst compiles are serialised through `compileQueue`; a superseded preview
  settles with the `SUPERSEDED` marker rather than an error, so the caller can
  ignore it instead of showing a failure.
- `html` requests bypass the queue — they never wait on a Typst compile. The
  worker tracks the latest html id and bails before the asset work when a newer
  one has arrived.
- Styles reach the VFS through Vite's `?raw` suffix and `compiler.addSource`.
  Images, twemoji and remote images go in through `compiler.mapShadow`.
- `loadFonts(CORE_FONTS, { assets: false })` — `assets: false` suppresses
  typst.ts's default jsdelivr bundle, which would break the offline guarantee.
- Fonts can only be *set* after init through `createTypstFontBuilder()` +
  `compiler.setFonts()`, not appended, which is why `ensureScriptFonts` rebuilds
  the whole set when a document turns out to contain CJK.

## The paged preview

Typst emits vector IR; `@myriaddreamin/typst-ts-renderer` turns it into one
composite SVG; `svg-split.ts` cuts it into a `<svg>` per `<g class="typst-page">`.

Those nodes are `adoptNode`d into the **live document**, not a shadow root. A
`<script>` from `DOMParser` is inert, but event-handler attributes are not, so
`svg-utils.ts scrub()` strips `on*`, `<script>`, `<foreignObject>` and
`javascript:` URLs before adoption.

The compile is gated on `previewMode === 'pages' && showPreview`. Nothing
recompiles for a hidden pane.

## The Web preview

`HtmlPreview.svelte` mounts the fragment in a shadow root. It keeps the
`<style>` node across renders — re-parsing 600 lines of CSS and an embedded
font on every keystroke was measurable — and restores focus afterwards, because
replacing the root's children destroys the focused element.

Both listeners (`click` for code copying and anchor navigation, `change` for
checkboxes) are bound once at `attachShadow` time, so they survive the swap.
`change` rather than `click`: it fires for keyboard activation, reports the
post-toggle state, and is `composed: false`, so it cannot leak into the page.

Nothing is hoisted out of the shadow root. The engine emits its script only
when `standalone` is set.

## Fonts

`fonts/` in the repo holds IBM Plex Sans, NewCM Math and DejaVu Sans Mono; the
Vite plugin copies them into `web/static/fonts/` and downloads Noto Sans SC/KR
there once, on the first build. `web/static/fonts` is gitignored.

The CJK faces are ~8 MB per weight, so they are excluded from the PWA precache
and fetched by the worker only once a document actually contains that script —
by script, so a Chinese document does not also pull the Korean face.

## Tests, and what each one is for

- `engine/src/html/tests.rs` — the bulk. Beyond the per-feature tests: a
  **cross-renderer parity** suite (both renderers must make the same
  keep-or-drop decision), a **nothing-disappears** sweep (every heading, task
  and code block in every fixture appears in both outputs), and **structural
  XSS guards** (no tag or attribute outside a known set, over every fixture).
  The parity and sweep tests exist specifically because a per-side test suite
  is what let the two renderers drift.
- `tests/check_html.py` — checks the shipped artefact: WCAG AA contrast for
  every colour token including the syntax ones, in both themes; an `alt` on
  every image; every in-page link resolving; no external resource, no inline
  handler, no `javascript:`/`data:` anchor.
- `web/tests/*` — vitest, node environment, for the pure pieces only.
- `web/e2e/*` — Playwright. The swallowed-headline bug lived in the rendered
  document; no unit test could have seen it. These load the app, type, switch
  tabs and read the shadow root.

## Conventions

- Prettier is authoritative: tabs and semicolons in `.ts`, two spaces and no
  semicolons in `.svelte`. `npm run format:check` runs in CI.
- Svelte 5 runes throughout. `$state`, `$derived`, `$effect`, `$props`.
- Comments explain *why*. What the code does is the code's job.
- `npm run check` is expected at 0 errors **and** 0 warnings.
