---
status: draft
owner: md2pdf
last_reviewed: 2026-05-16
product_name: md2pdf
scope: engineering
stack: sveltekit_adapter_static_prerender
tags: [sveltekit, adapter-static, prerender, typst, wasm, worker, typst-ts-renderer, svelte5, html]
---

# md2pdf — Engineering

## 0. Hard constraints (don't cross these lines)

- **Fully static**: SvelteKit + `@sveltejs/adapter-static`. The build output drops onto any static host (`svelte.config.js`).
- **No server features**: no `+server.ts`, no form actions.
- **prerender boundary**: prerender executes route rendering at build time. Any browser API (`window`, `document`, `indexedDB`, `caches`, …) must be reached only inside `onMount()` or behind `if (browser)`.
- **Heavy work in workers**: Typst compilation runs in a Web Worker so the UI thread stays responsive.
- **Offline by default**: all fonts and twemoji SVGs are bundled into `static/`. The only network calls during a compile are user-supplied remote image URLs.

---

## 1. Commands

- Install: `npm install`
- Dev: `npm run dev`
- Type check: `npm run check`
- Build (static): `npm run build`
- Preview built output: `npm run preview`

---

## 2. Directory layout (current)

```
src/
  routes/
    +layout.ts                       # global prerender + trailingSlash
    +layout.svelte
    +page.svelte                     # PDF editor — the homepage
    +page.ts                         # prerender: true
    cards/+page.svelte               # Cards mode
    cards/+page.ts
    slides/+page.svelte              # Slides mode
    slides/+page.ts
  lib/
    components/
      PdfEditor.svelte               # PDF mode: editor / SVG preview / export / settings / CORS modal
      CardsEditor.svelte             # Cards mode: per-page compile
      SlidesEditor.svelte            # Slides mode: per-page compile
      CardGallery.svelte             # Card/slide gallery (SVG blobs)
      EditorPane.svelte              # Wraps Milkdown WYSIWYG + CodeMirror plain editor
      DocumentMenu.svelte            # Document picker (recent + templates)
      StatusHint.svelte              # Floating "Updating preview" pill
    pipeline/
      markdownToTypst.ts             # mdast → Typst; markdownToTypstPages() for per-page output
      plugins/
        remark-mark.ts               # ==highlight==
        remark-admonitions.ts        # :::success / :::warning / :::tip / :::info / :::danger
        remark-spoiler.ts            # +++++ … +++++
        remark-twemoji.ts            # unicode emoji → twemoji nodes
        remark-emoji-shortcodes.ts   # :innocent: → 😇 (then twemoji)
        remark-pagebreak-token.ts    # [[pagebreak]] → custom node
        remark-simple-supersub.ts    # ^sup^ / ~sub~
    workers/
      typstClient.ts                 # main-thread wrapper (compilePdf / compileVector)
      typst.worker.ts                # WASM init, fonts, VFS, compile queue
    typst/
      admonitions.typ                # admonition / spoiler / task-item / list-marker helpers
      styles/*.typ                   # one entry-point article(...) per style
      renderer.ts                    # typst.ts SVG renderer (lazy WASM)
      svg-utils.ts                   # per-page SVG extraction
    mermaid/render.ts                # mermaid → SVG bytes
    twemoji/loader.ts                # scan markdown → fetch needed twemoji SVGs
    stores/
      documentStore.svelte.ts        # IndexedDB-backed document persistence
      settingsStore.svelte.ts        # liveUpdate, pageNumbers, corsProxy
    utils/
      image-utils.ts                 # local image helpers
      remote-images.ts               # http(s) image fetcher (with CORS-proxy fallback)
    templates/
      pdf-templates.ts               # WELCOME + RESUME + AI_CHAT + NOTION
      card-templates.ts
      slides-templates.ts
  hooks.client.ts                    # intentionally empty (no analytics)
static/
  fonts/                             # bundled Typst fonts
  twemoji/                           # bundled emoji SVGs
docs/                                # design + engineering notes
```

---

## 3. Routes & prerender

### 3.1 Global prerender

- `src/routes/+layout.ts`:
  - `export const prerender = true`
  - `export const trailingSlash = 'always'`
- All page-level `+page.ts` files simply `export const prerender = true;`

### 3.2 No dynamic segments

Routes are now literal paths (`/`, `/cards/`, `/slides/`). No `EntryGenerator` is required.

---

## 4. Markdown → Typst → PDF

### 4.1 Markdown → Typst (pure function on main thread)

Implementation: `src/lib/pipeline/markdownToTypst.ts`

- Parse stack: `unified + remark-parse + remark-frontmatter + remark-gfm + remark-math` plus the custom plugins listed above.
- Two entry points:
  - `markdownToTypst(md, options)` → one Typst source for the whole document.
  - `markdownToTypstPages(md, options)` → an array of Typst sources, one per page (cards/slides).
- Output: a `main.typ` string that begins with `#import "styles/<style>.typ": article`, `#import "/admonitions.typ": admonition, spoiler, task-item`, and `#show: article.with(...)`.

### 4.2 Typst compile (Worker)

- Client wrapper: `src/lib/workers/typstClient.ts`
- Worker entry: `src/lib/workers/typst.worker.ts`

Message protocol:

- request: `{ type: 'compile', id, markdown, images, pageNumbers, format }`
  (`format` = `'pdf' | 'preview'`), or
  `{ type: 'html', id, markdown, images, standalone }`
- response: `{ type: 'compile-result', id, ok, pdf? | preview? | html?, diagnostics, error? }`

`html` requests bypass `compileQueue` — they run the engine directly and never
wait on, or block, a Typst compile.

Notes:

- The worker serializes compilations through `compileQueue` to keep the Typst compiler's state consistent.
- Style files are imported via Vite's `?raw` suffix and added to the VFS through `compiler.addSource('/styles/xxx.typ', ...)`.
- Images, twemoji SVGs, and remote images are injected via `compiler.mapShadow('/' + path, bytes)`.
- `loadFonts(CORE_FONTS, { assets: false })` — the `assets: false` is required to suppress typst.ts's default jsdelivr font bundle.

### 4.3 Vite worker conventions

- Worker creation (already wrapped in `TypstWorkerClient`):

```ts
new Worker(new URL('./typst.worker.ts', import.meta.url), { type: 'module' });
```

- `vite.config.ts` sets `worker.format = 'es'`.
- `vite.config.ts` also defines two custom plugins:
  - `md2pdf-copy-twemoji` — copies `node_modules/twemoji-emojis/vendor/svg/` to `static/twemoji/` on first build.
  - `md2pdf-bundle-fonts` — downloads any missing entries from `FONTS_TO_BUNDLE` to `static/fonts/` on first build.

---

## 5. SVG preview (typst.ts renderer)

Implementation: `src/lib/typst/renderer.ts` + `src/lib/typst/svg-utils.ts` + each Editor component.

- `getTypstRenderer()` lazy-loads `@myriaddreamin/typst-ts-renderer` WASM (~1 MB), first use only.
- Flow: Typst produces vector IR (`format: 'vector'`) → `renderer.renderSvg()` returns a composite SVG → `extractPageSvgs()` splits it into one `<svg>` per `<g class="typst-page">`.
- PDF mode renders all pages into a single scrolling stack.
- Cards/slides mode uses `markdownToTypstPages()` for per-page compile + incremental update (only recompile changed pages).
- PDF export runs `client.compilePdf()` on demand to a `Blob` URL.

---

## 6. Fonts & offline guarantee

### 6.1 Fonts (current behaviour)

- `static/fonts/` ships these:
  - **Core:** `IBMPlexSans-{Regular,Bold}`, `NewCMMath-{Regular,Book}`
  - **Latin/serif/mono backup** (formerly the typst-assets "text" bundle): `DejaVuSansMono-{Regular,Bold,Oblique,BoldOblique}`, `LibertinusSerif-{Regular,Bold,Italic,BoldItalic,Semibold}`
  - **CJK (lazy)**: `NotoSansCJKsc-{Regular,Bold}`, `NotoSerifSC-Regular`
  - **Emoji (lazy)**: `NotoColorEmoji`
- The Vite plugin `md2pdf-bundle-fonts` downloads any missing files on first build/dev — once.
- The worker calls `loadFonts(urls, { assets: false })` so typst.ts does NOT auto-load anything from jsdelivr.

### 6.2 Analytics

`src/hooks.client.ts` is intentionally empty. No analytics SDK is installed.

### 6.3 Network calls during runtime

Aside from same-origin assets, the only network call during a compile is:

- Remote image URLs the user wrote into the markdown.
  - On CORS error, the optional user-configured proxy is consulted (see Settings → CORS proxy modal).
  - On total failure, the image is silently dropped; the user can paste/drop the file manually.

---

## 7. Mermaid

Implementation: `src/lib/mermaid/render.ts` + Mermaid pre-pass in `PdfEditor.svelte`.

- `renderMermaidToSvg(code, id)` returns SVG bytes.
- The Mermaid pre-pass scans for fenced ```` ```mermaid ```` blocks, renders each to SVG, writes the bytes to `images['mermaid-<n>.svg']`, and rewrites the block to `![Mermaid Diagram](mermaid-<n>.svg)` so the standard image pipeline picks it up.

---

## 8. Twemoji

- The `md2pdf-copy-twemoji` Vite plugin mirrors `node_modules/twemoji-emojis/vendor/svg/` into `static/twemoji/`.
- `src/lib/pipeline/plugins/remark-twemoji.ts` walks `text` nodes and replaces matched emoji with `{ type: 'twemoji', codepoint }` nodes.
- `src/lib/pipeline/plugins/remark-emoji-shortcodes.ts` runs *before* twemoji and expands `:innocent:` → 😇 via `node-emoji`.
- `src/lib/twemoji/loader.ts` mirrors the same regex + shortcode logic on the raw markdown so that `PdfEditor.compile` can fetch the needed SVGs and inject them into the worker's `images` map.
- The renderer emits `#box(baseline: 0.15em, height: 1em, image("twemoji/<codepoint>.svg"))` for each twemoji node.

---

## 9. Settings

`src/lib/stores/settingsStore.svelte.ts` (Svelte 5 runes, localStorage-backed):

- `liveUpdate: boolean` — gates the auto-compile effect. When off, the toolbar shows an "Update" button and `Ctrl/Cmd+Enter` triggers a compile from anywhere.
- `pageNumbers: boolean` — default for the `set page(numbering:)` toggle; frontmatter `pageNumbers:` overrides.
- `corsProxy: string` — optional proxy URL. The image loader calls it as `${proxy}?url=<encoded>` (or `${proxy}&url=...` if the proxy already contains `?`).

---

## 10. HTML target

### 10.1 Why the engine renders it, not Typst

Typst's own HTML export (`--format html`) works in the `typst` binary but not
in the browser: `typst-ts-web-compiler`'s `get_artifact` only knows vector and
PDF, and `TypstWorld.compileHtml()` returns diagnostics without ever handing
back the HTML string (checked in 0.6.1-rc5, 0.7.0 and 0.8.0-rc3). HTML export
also rejects `grid`/`rect`/`place`/`block`, so every template in
`package/styles` would need an HTML twin regardless.

So the engine renders HTML itself, from the same comrak parse as the Typst
markup. One implementation, identical bytes on both front-ends — and the
browser skips the Typst compile entirely, which is why the view can update on
a flat 120 ms debounce instead of the adaptive 450–2500 ms compile one.

### 10.2 The two engine calls

```
html_images(md)        -> local image paths, one per line
remotes(md)            -> url<TAB>alias, one per line   (already existed)
twemojis(md)           -> codepoints, one per line      (already existed)
html_mermaid(md)       -> key<TAB>escaped-source, one per line

render_html(md, options, manifest, assets) -> the document
```

The host resolves every key it can, concatenates the bytes and describes the
slices in `manifest` (`key<TAB>byte-length` lines). `options` is a `key=value`
block; `standalone=1` wraps the fragment in a full document. The engine
base64s each asset into a `data:` URI, so the output needs no sidecar files.

- Browser: `web/src/lib/workers/typst.worker.ts` — `renderHtml()`, outside the
  compile queue. `loadPlugin()` instantiates `engine.wasm` (and, lazily,
  `typst_mmdr.wasm` for diagrams) with a hand-written `wasm-minimal-protocol`
  host. `web/src/lib/workers/assetBundle.ts` holds the pure wire-format code.
- CLI: `package/lib.typ` — `prepare-html()`, published as
  `#metadata(...) <md2pdf-html>` and pulled out with `typst eval`, so no
  `--features html` and no experimental Typst flag is involved.

The math faces in `package/fonts/` are derived from `fonts/NewCMMath-Book.otf`
and committed like `engine.wasm`. Regenerate them with `hb-subset` (harfbuzz)
plus `woff2_compress` after changing the ranges:

```sh
hb-subset --output-file=math.otf --layout-features=ssty,kern,aalt --desubroutinize \
  --unicodes=U+0020-007E,U+00A0-00FF,U+0370-03FF,U+2000-23FF,U+2500-27BF,U+2A00-2AFF,U+FE00-FE0F \
  fonts/NewCMMath-Book.otf && woff2_compress math.otf
hb-subset --output-file=math-alpha.otf --layout-features=ssty,kern,aalt --desubroutinize \
  --unicodes=U+1D400-1D7FF fonts/NewCMMath-Book.otf && woff2_compress math-alpha.otf
```

The second file holds the math alphanumerics (`\mathbb`, `\mathfrak`, …). It is
half the weight and most documents never reach into that block, so the engine
embeds it only when the rendered MathML actually contains one of those
characters.

`bin/md2pdf` is the host side: stdlib-only Python 3.9+, portable to Windows.
It writes a `main.typ` next to the document (so `read()`/`image()` resolve
against the document root), runs `typst eval` to discover remote image URLs,
fetches them into a per-run temp directory, exposes that as `<docdir>/remote`
(symlink, falling back to a copied directory where symlinks need privileges),
then renders. Nothing is cached between runs: the alias the engine emits is a
32-bit hash of the URL, so a shared cache would let any document read another's
downloads by naming `remote/<hash>` directly.

### 10.3 Styling

`engine/src/html/css.rs` is the whole stylesheet, inlined into every document.
Tokens are declared once with `light-dark()`; `color-scheme` decides the side,
so the OS preference and an explicit `data-theme` share one set of values.
`data-theme` is honoured on the fragment root, the document root or the shadow
host — the editor sets it on the host, the export never sets it at all.

The preview mounts the fragment in a shadow root
(`web/src/lib/components/HtmlPreview.svelte`) so the document's CSS and the
app's CSS cannot reach each other.

The export carries one script: copy-to-clipboard on code blocks, plus
in-document links. It binds a single listener to `document` and resolves its
scope from `e.composedPath()[0].getRootNode()`, which is what makes it work in
both places — inside a shadow root the browser cannot resolve a `#fragment`
(the ids are not in the document) and `document.currentScript` is null, so the
script cannot find its own root. A `<script>` that arrives via `innerHTML` is
inert, so `HtmlPreview` re-executes it once at document level.

The outline drawer opens with a checkbox and needs no script at all;
`toc: false` in the frontmatter drops the button and drawer entirely, without
touching an explicit `[toc]` in the body.

### 10.4 What HTML does not do

Cover page, DIN 5008 letter mode, running header/footer and page numbers are
page concepts and are skipped. `[[pagebreak]]` becomes a divider. Citations
render as an IEEE-shaped list built in Rust rather than through Typst's CSL, so
they will not match the PDF exactly.

---

## 11. Known gaps

- Preflight / one-click fixes for shaky AI markdown: not implemented.
- Asset manager UI: images stored in IndexedDB but no visible list/delete UI.
- `/Creator` and `/Producer` PDF metadata fields are still set by Typst itself (e.g. `Typst 0.13.1`). Stripping them would require either byte-patching the PDF or an upstream Typst change; neither is in scope.
