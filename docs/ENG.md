# md2pdf engineering notes

Working notes for the code as it is now. [DESIGN.md](DESIGN.md) explains why
it is shaped this way.

## Hard constraints

- **No server.** SvelteKit with `@sveltejs/adapter-static`, everything
  prerendered. No `+server.ts`, no form actions, no runtime host.
- **Prerender boundary.** Route modules run at build time. `window`,
  `document`, `indexedDB` and `caches` are only reachable inside `onMount()`
  or behind `if (browser)`.
- **Offline after first load.** Fonts, the Typst package, the engine and the
  Twemoji mirror are all same-origin. At runtime the only outbound requests
  are image URLs the document asks for.
- **One Markdown implementation.** A change to what Markdown means belongs in
  `engine/`. Anywhere else is the wrong place.

## Layout

```
engine/                       Rust, one crate, two renderers
  rust-toolchain.toml         pinned toolchain, so engine.wasm rebuilds byte for byte
  src/lib.rs                  the plugin ABI, asset discovery, helpers both renderers share
  src/preprocess.rs           pre-parse passes: :::blocks, spoilers, + widths,
                              lettered lists, HedgeDoc/Outline syntax, citations
  src/typst.rs                Typst markup, the PDF renderer
  src/frontmatter.rs          the one YAML frontmatter parser
  src/rawhtml.rs              which raw HTML tags both renderers honour
  src/emoji.rs                emoji sequences -> Twemoji file names
  src/twemoji.txt             the Twemoji file list (a test checks it matches package/twemoji)
  src/html/mod.rs             the HTML renderer
  src/html/highlight.rs       the syntax highlighter
  src/html/css.rs             the document stylesheet and the export's script
  src/html/tokens.rs          colour tokens and labels, shared with the Typst side
  src/html/assets.rs          the key<TAB>len wire format for asset blobs
  src/html/math.rs            math-core bridge, and the check for what the PDF can typeset
  src/html/math-corpus.tsv    682 formulas with the PDF's verdict on each (a parity test)
  src/html/tests.rs           the bulk of the tests, incl. the cross-renderer parity suite

package/                      the `md2pdf` Typst package
  lib.typ                     prepare() for PDF, prepare-html() for the CLI
  styles/modern-tech.typ      the one document style
  cover.typ                   cover page geometries and palettes
  furniture.typ               running header and footer
  admonitions.typ             callouts, spoilers, task items
  tokens.typ                  reads the engine's colour tokens
  engine.wasm                 committed; rebuilt by ./build.sh
  vendor/                     mitex (math), mmdr (Mermaid)
  twemoji/                    3689 SVGs

bin/md2pdf                    the CLI and local stdio MCP server; Python stdlib only
tests/                        Markdown fixtures, test_cli.py, check_html.py,
                              visual.py and golden/

web/src/
  env.ts                      build-time env vars (PUBLIC_SITE_URL)
  routes/                     /  and  /reference, nothing else
  lib/components/
    PdfEditor.svelte          state, wiring, layout
    EditorPane.svelte         wraps the editor, owns image paste/drop
    MarkdownEditor.svelte     CodeMirror 6
    HtmlPreview.svelte        the shadow-root mount for the Web view
    DocumentMenu.svelte       all local documents, delete with confirmation
    ShortcutOverlay.svelte    the cheatsheet
    StatusHint.svelte         the floating "Updating preview" pill
  lib/preview/
    previewController.svelte.ts  both previews: scheduling, error and warning
                              state, which images go to the worker
    page-slots.ts             mounts only the pages near the viewport
  lib/workers/
    typst.worker.ts           Typst compiler and the engine, both in the worker
    typstClient.ts            the main-thread wrapper
    compileProtocol.ts        every message type, shared by both sides
    assetBundle.ts            decodes the engine's asset wire format
    remoteImages.ts           remote-image fetch, limits and cache
    diagnostics.ts            turns Typst errors into readable messages
  lib/editor/commands.ts      slash commands and the formatting keymap
  lib/typst/                  vector IR -> per-page SVG, and the SVG scrubber
  lib/storage/documents.ts    IndexedDB
  lib/stores/                 document and settings state (Svelte 5 runes)
  lib/stores/tabSync.ts       live sync between tabs (BroadcastChannel)
  lib/exports.ts              PDF, HTML and Markdown downloads
  lib/shortcuts.ts            the app's keyboard shortcuts
  lib/templates/              the welcome document, imported from tests/extended.md
  lib/utils/                  image helpers, task-marker parsing, focus trap
```

Web code imports app modules as `#lib/…` (an `imports` map in
`web/package.json`). There is no `svelte.config.js`. The SvelteKit config
lives in `web/vite.config.ts`.

## Commands

```sh
./build.sh                     # rebuild package/engine.wasm, install @local/md2pdf
./build.sh engine              # only rebuild package/engine.wasm
./build.sh install             # only install the package
cd engine && cargo test        # 219 engine tests
cargo fmt --check && cargo clippy --all-targets -- -D warnings
python3 -m unittest discover -s tests -p "test_*.py"   # 48 CLI tests
python3 tests/visual.py [--update]                      # render goldens
python3 tests/check_html.py out.html
cd web && npm install
npm run check                  # svelte-check; must be 0 errors and 0 warnings
npm test                       # vitest
npm run test:e2e               # Playwright, real browser
npm run format:check           # Prettier
npm run format                 # Prettier, rewrite files
npm run dev
npm run build                  # -> web/build/
```

The Rust toolchain, with the `wasm32-unknown-unknown` target, is pinned in
`engine/rust-toolchain.toml`. `build.sh` remaps machine-specific paths, so the
wasm build is reproducible. **Any commit that touches `engine/` must re-run
`./build.sh engine` and commit `package/engine.wasm`.** The web build has no
Rust toolchain and reads the committed file. CI rebuilds it and fails if the
result differs.

## The engine ABI

The engine uses `wasm-minimal-protocol`. The Typst package calls these
functions through `plugin()`, and the worker calls the same module through a
hand-written host in `typst.worker.ts`.

| Function | Returns |
|---|---|
| `convert(md, strip_h1, citations, remote_dir)` | Typst markup |
| `render_html(md, options, manifest, assets)` | the HTML fragment or document |
| `html_assets(md)` | image, remote, emoji, font and Mermaid rows, from one parse |
| `frontmatter(md)` | the frontmatter as JSON, keys normalised |
| `leading_h1(md)` | the first H1's text, for the title rule |
| `twemojis(md)` | the Twemoji codepoints the document uses |
| `tokens()` | the colour palette and labels, so Typst and HTML agree |
| `inline_bibliography(md)` / `without_inline_bibliography(md)` | the citation split |

`html_assets` answers every "what must the host fetch" question from one
parse. It emits one `kind<TAB>value…` row per asset, where `kind` is `image`,
`remote` (`url<TAB>alias`), `emoji`, `font` or `mermaid`
(`key<TAB>escaped-source`). Both `lib.typ` and the worker use it.

`frontmatter` returns the block as JSON with keys lowercased and `_` read as
`-`. Scalars keep the text the author wrote; only booleans and nulls get a
type. Typst reads this instead of calling `yaml()`, so the PDF and HTML see
the same values.

`render_html` options are `key=value` lines:

- `standalone` wraps the fragment in a full document (the download).
- `editable` adds `data-md-line` and live checkboxes. Only the preview sets
  it, never a download.
- `theme` fixes the initial standalone colour scheme to `light` or `dark`.
  `system` leaves it automatic.
- `remote-dir` names the directory of remote-image aliases (default
  `remote`). The CLI passes its per-run folder.

`manifest` is `key<TAB>byte-length` lines that say how to slice `assets`, the
concatenated bytes of every image, Twemoji SVG, font and Mermaid diagram the
host resolved.

In `lib.typ`, `prepare()` returns `(skip, remotes, images, body, template,
scope)`, where `images` lists the local image paths the document uses. Both
`prepare()` and `prepare-html()` take `remote-dir` (where the host put remote
images) and `missing-images` (local paths the host cannot supply, each
rendered as a placeholder).

`md2pdf mcp --root PATH` is a JSON-RPC server over stdio, one message per
line. It runs each render as a child CLI process with captured output, so
nothing the render prints reaches the protocol stream. File paths must stay
inside the root. The syntax reference and example document are fixed
resources.

## Line origins

The preprocess passes do not keep line numbers. Blank runs collapse to three
lines, `:::` blocks and spoilers collapse to a placeholder and re-parse their
body as a fresh document, and `+`-width tables insert a line. So the passes
carry `(String, u32)`: the text plus the 1-based line it came from, `0` for
lines a pass made up. `render_source` maps its origins back to the original
document once, up front.

This is what makes `data-md-line` mean "the line the author wrote", not "the
line after preprocessing", and the task-checkbox write depends on it. **The
Typst renderer ignores these fields.**

The engine still tags every top-level block, not only task items. That used to
feed editor/preview scroll sync, which has been removed. The attribute costs a
few bytes in the preview, never appears in a download, and keeps the option
open if sync is tried again.

## The worker

```ts
// requests
{ type: 'compile', id, markdown, images?, format?: 'pdf' | 'preview' }
{ type: 'html',    id, markdown, images?, standalone?, theme?, editable? }
{ type: 'cancel',  id }
// responses
{ type: 'compile-result', id, ok, pdf? | preview? | html?, diagnostics, error? }
{ type: 'remotes-ready' }
```

All message types live in `compileProtocol.ts`.

- Typst compiles run one at a time through a queue. A newer preview replaces
  a queued one. A dropped preview settles with the `SUPERSEDED` marker instead
  of an error, so the caller ignores it rather than showing a failure.
- `html` requests skip the queue and never wait for a Typst compile. The
  worker tracks the latest preview id and stops before the asset work when a
  newer one has arrived. A download is always answered.
- Requests carry only new or changed images. The worker keeps every image it
  has seen, from either kind of request, in `imageStore`. Every compile maps
  the store into the Typst VFS through `mapOnce`, which compares by identity.
  Unchanged files are not re-mapped, so Typst keeps its caches for them.
- Local images the document names but the store lacks are passed to
  `prepare()` as `missing-images` and render as placeholders.
- Remote images: the worker takes the `remote` rows from `html_assets` and
  fetches them (`remoteImages.ts`: 32 MB, 20 s, 64 images, 6 in parallel, no
  credentials). A preview renders without them right away, and the worker
  posts `remotes-ready` once they arrive so the preview renders again. PDF
  export and the HTML download wait for them.
- The package arrives through `manifest.json`: `.typ` files go in through
  `compiler.addSource`, `.wasm` files through `compiler.mapShadow`.
  `engine.wasm` is fetched once and used both by Typst and by the worker's
  own engine calls.
- `loadFonts(CORE_FONTS, { assets: false })`. `assets: false` stops typst.ts
  from loading its default font bundle from jsDelivr, which would break the
  offline guarantee.
- After init, fonts can only be replaced as a whole set
  (`createTypstFontBuilder()` and `compiler.setFonts()`), not added to. That is
  why `ensureScriptFonts` rebuilds the set when a document turns out to contain
  CJK.
- `diagnostics.ts` strips paths to the generated entry file and the package
  from Typst errors, because they mean nothing to the author.

## The paged preview

Typst emits vector IR. `@myriaddreamin/typst-ts-renderer` turns it into one
composite SVG, and `svg-split.ts` cuts that into one `<svg>` per
`<g class="typst-page">`. `page-slots.ts` mounts only the pages near the
viewport.

Those nodes are adopted into the **live document**, not a shadow root. A
`<script>` from `DOMParser` is inert, but event-handler attributes are not, so
`scrub()` in `svg-utils.ts` strips `on*`, `<script>`, `<foreignObject>` and
`javascript:` URLs before adoption.

The compile only runs while the Pages tab is on screen. Nothing recompiles for
a hidden pane.

## The Web preview

`HtmlPreview.svelte` mounts the fragment in a shadow root. It keeps the
`<style>` node across renders, because re-parsing the stylesheet and an
embedded font on every keystroke was measurably slow. It restores focus
afterwards, because replacing the root's children destroys the focused
element.

Both listeners are bound once, when the shadow root is attached, so they
survive the swap: `click` for code copying and anchor navigation, `change` for
checkboxes. `change` rather than `click` because it fires for keyboard
activation, reports the state after the toggle, and is `composed: false`, so
it cannot leak into the page.

Nothing is hoisted out of the shadow root. The engine emits its script only
when `standalone` is set.

## Storage

IndexedDB database `md2pdf_documents`, version 2:

- `documents`: one record per document (id, name, content, timestamps), with
  no images.
- `assets`: the images of each document that has any, keyed by document id.

Version 1 kept images inside each record, so listing documents loaded every
image of every document. The upgrade to version 2 moves the embedded images
into `assets`. The document list never loads images.

`tabSync.ts` broadcasts every edit to other tabs on a `BroadcastChannel`. A
tab showing the same document takes the edit if it is newer than its own last
edit, so two tabs typing at once settle on the last keystroke.

## Fonts and the PWA

`fonts/` in the repo holds IBM Plex Sans, NewCM Math and DejaVu Sans Mono. A
Vite plugin copies them into `web/static/fonts/`. It also downloads Noto Sans
SC and KR once into `web/.cache/cjk` and copies them from there on each build.
`web/static/fonts` is gitignored.

The service worker precaches the compilers, the md2pdf package (`.typ` and
`.wasm`) and the Latin and math fonts, about 40 MB in total. The Typst
compiler alone is about 30 MB. Twemoji (about 3700 files, 17 MB) and the CJK
faces (about 8 MB per weight) are not precached. The worker fetches them only
when a document uses them, and picks CJK faces by script, so a Chinese
document does not also load the Korean face.

## Tests, and what each is for

- `engine/src/html/tests.rs` and the other `#[cfg(test)]` modules: 219
  cargo tests. Besides the per-feature tests there are:
  - a **cross-renderer parity** suite: both renderers must make the same
    keep-or-drop decision.
  - a **nothing-disappears** sweep: every heading, task and code block in
    every fixture appears in both outputs.
  - **structural XSS guards**: no tag or attribute outside a known set, over
    every fixture.
  - a **math corpus** test: for each formula in `math-corpus.tsv`, the engine
    must predict correctly whether the PDF can typeset it.
  - a test that `twemoji.txt` matches `package/twemoji`.

  The parity and sweep tests exist because separate test suites per renderer
  are what let the two drift apart.
- `tests/test_cli.py`: 48 tests for `bin/md2pdf`, including batch mode and
  the MCP server.
- `tests/visual.py` with `tests/golden/`: renders every `tests/*.md` through
  the CLI with bundled fonts only and compares the pages with committed
  images. `--update` rewrites them.
- `tests/check_html.py`: checks an exported file. WCAG AA contrast for every
  colour token including the syntax ones, in both themes; an `alt` on every
  image; every in-page link resolves; no external resource, no inline handler,
  no `javascript:` or `data:` link.
- `web/tests/*`: vitest for the pure pieces, including the IndexedDB v1 to v2
  migration (with `fake-indexeddb`).
- `web/e2e/*`: Playwright specs `app`, `cjk`, `degrade`, `math`, `export`,
  `documents`, `layout` and `offline`. They load the app, type, switch tabs
  and read the shadow root. Some bugs, like a headline swallowed in the
  rendered document, only show up there.

Fixtures include `hedgedoc.md`, `outline.md` and `regressions.md` for the
imported syntax and for inputs that once broke a render.

CI (`.github/workflows/ci.yml`) has five jobs: `engine`, `cli`, `render`,
`web-lint` and `web-e2e`.

## Conventions

- Prettier is authoritative: tabs and semicolons in `.ts`, two spaces and no
  semicolons in `.svelte`. CI runs `npm run format:check`.
- Svelte 5 runes throughout: `$state`, `$derived`, `$effect`, `$props`.
- Comments explain why. The code says what it does.
- `npm run check` is expected at 0 errors **and** 0 warnings.
