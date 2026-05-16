---
status: draft
owner: md2pdf
last_reviewed: 2026-05-16
product_name: md2pdf
scope: engineering
stack: sveltekit_adapter_static_prerender
tags: [sveltekit, adapter-static, prerender, typst, wasm, worker, typst-ts-renderer, svelte5]
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

- request: `{ type: 'compile', id, mainTypst, images, format? }` (`format` = `'pdf' | 'vector'`)
- response: `{ type: 'compile-result', id, ok, pdf? | vector?, diagnostics, error? }`

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

## 10. Known gaps

- Preflight / one-click fixes for shaky AI markdown: not implemented.
- Asset manager UI: images stored in IndexedDB but no visible list/delete UI.
- `/Creator` and `/Producer` PDF metadata fields are still set by Typst itself (e.g. `Typst 0.13.1`). Stripping them would require either byte-patching the PDF or an upstream Typst change; neither is in scope.
