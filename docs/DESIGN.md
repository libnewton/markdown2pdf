---
status: draft
owner: md2pdf
last_reviewed: 2026-05-16
product_name: md2pdf
tech_route: typst_wasm_client_only
tags: [prd, markdown, export, pdf, client-only, typst, wasm, sveltekit]
---

# md2pdf — Design

## 0. One-liner

md2pdf is a fully static site: in the browser, it converts Markdown to Typst, compiles to a vector IR with Typst WASM in a Web Worker, renders that IR to per-page SVGs for live preview, and produces a PDF on demand.

> This document distinguishes *what's implemented* from *what's planned*. The code is the source of truth; this doc lags it.

---

## 1. Current scope

### 1.1 Implemented

- **Editor & export**: left-pane Markdown editor (WYSIWYG via Milkdown or CodeMirror plain mode), right-pane SVG preview, one-click PDF download (`src/lib/components/PdfEditor.svelte`).
- **Three modes**: PDF (`/`), Cards (`/cards/`), Slides (`/slides/`). All English-only.
- **Single PDF style** (`modern-tech`); cards and slides each ship multiple themes.
- **Mermaid**: fenced ```` ```mermaid ```` blocks are rendered to SVG client-side, then injected into the Typst VFS as images.
- **Twemoji** for emoji rendering — bundled locally at `static/twemoji/`. Both unicode emoji (😀) and shortcodes (`:innocent:`) are recognized.
- **Remote images**: `![](https://...)` URLs are fetched and injected into the VFS. CORS failures fall back to an optional user-configured proxy.
- **Pipeline**: Markdown (mdast) → Typst source → Typst(WASM) → vector IR / PDF (`src/lib/pipeline/markdownToTypst.ts`, `src/lib/workers/*`).

### 1.2 Intentionally out of scope (right now)

- Preflight (lint + one-click fixes for shaky AI markdown)
- A stable Document IR layer above mdast
- Generic asset import beyond images (audio, video, etc.)

---

## 2. Markdown profile

### 2.1 Frontmatter (YAML)

Parsed by hand in `src/lib/pipeline/markdownToTypst.ts` (`parseFrontmatter*`).

- `title: string`
- `author: string` or `authors: string[]` (inline `[a, b]` or YAML list form)
- `lang: zh | en` (PDF `lang` attribute only; UI is English-only)
- `pageNumbers: bool` — overrides the menu toggle when present

Title precedence: `options.title` → `frontmatter.title` → first leading H1.

### 2.2 Block syntax

- Headings H1–H6
- Paragraphs
- Ordered and unordered lists, **nested with marker cycles**:
  - Unordered: `•` → `▪` → `◦`
  - Ordered: `1.` → `a)` → `i)`
- Task lists `- [ ]` / `- [x]` render as real checkboxes (no bullet)
- Blockquotes
- Fenced code blocks **with line numbers in the gutter**
- Tables (GFM)
- Math blocks (`$$ … $$`)
- Themed admonitions: `:::success`, `:::warning`, `:::tip`, `:::info`, `:::danger`, `:::note`
- Spoilers: `+++++ … +++++` (always-expanded ▶-marker block)
- Thematic break (`---`)
- Page break: `[[pagebreak]]`
- TOC: a paragraph whose entire content is `[toc]` (case-insensitive) becomes `#outline()`

### 2.3 Inline syntax

- Bold, italic, strikethrough (GFM)
- Inline code (subtly sized to match surrounding text)
- `==highlight==` (yellow background)
- Links (inline and reference)
- Inline math (`$…$`)
- Footnote references
- Super- and subscript (`^x^`, `~y~`)
- Unicode emoji & `:shortcode:` emoji (Twemoji)

### 2.4 Images

- `![alt](url)` — alt becomes a small centered caption beneath the image
- `![alt](url =200x200)` and `![alt](url "=200x200")` — explicit dimensions (HackMD style)
- Local images: pasted/dropped into the editor are stored in IndexedDB and injected into the VFS at compile time
- Remote images: fetched live; on CORS failure the optional proxy is consulted

---

## 3. System architecture

### 3.1 Data flow

```
Markdown (string)
  ├─ Mermaid pre-pass     ```mermaid → SVG bytes → ![](id.svg)
  ├─ Twemoji pre-scan     emoji + shortcodes → image bytes in VFS
  ├─ Remote image pre-fetch  http(s) → image bytes in VFS
  ├─ markdownToTypst        mdast → main.typ string
  └─ TypstWorkerClient.compileVector / compilePdf(main.typ, images)
        └─ typst.worker
            - Lazy WASM init (compiler + renderer)
            - Lazy font upgrades (CJK, emoji)
            - Inject /main.typ, /admonitions.typ, /styles/* and images into the VFS
            - compile → bytes + diagnostics

Vector IR
  ├─ typst.ts renderer → SVG, split by page for preview
  └─ Export: on-demand PDF compile → Blob URL → download
```

### 3.2 Module map

- UI
  - `src/lib/components/PdfEditor.svelte` — main editor, debounced compile, Mermaid preprocess, SVG rendering, settings (live update / page numbers / CORS proxy modal).
  - `src/lib/components/CardsEditor.svelte` — cards mode, page-by-page compile.
  - `src/lib/components/SlidesEditor.svelte` — slides mode, page-by-page compile.
- Pipeline
  - `src/lib/pipeline/markdownToTypst.ts` — unified parse + render. `markdownToTypstPages()` returns one Typst source per page (for cards/slides).
  - `src/lib/pipeline/plugins/` — custom remark plugins: `remark-mark` (==highlight==), `remark-admonitions`, `remark-spoiler`, `remark-twemoji`, `remark-emoji-shortcodes`, `remark-pagebreak-token`, `remark-simple-supersub`.
- Worker
  - `src/lib/workers/typstClient.ts` — main-thread wrapper (`compilePdf`, `compileVector`).
  - `src/lib/workers/typst.worker.ts` — Typst init, font loading, VFS, compile queue.
- Preview
  - `src/lib/typst/renderer.ts` — typst.ts SVG renderer (lazy WASM load).
  - `src/lib/typst/svg-utils.ts` — per-page SVG extraction.
- Typst templates
  - `src/lib/typst/styles/*.typ` — one `article(...)` entry per style.
  - `src/lib/typst/admonitions.typ` — shared helpers (`admonition`, `spoiler`, `task-item`, `md2pdf-list-markers`, `md2pdf-enum-numbering`).

---

## 4. Typst template system

### 4.1 Contract: styling lives in templates

`markdownToTypst` is allowed to:

1. `#import "styles/<style>.typ": article`
2. `#import "/admonitions.typ": admonition, spoiler, task-item`
3. `#show: article.with(title:, authors:, lang:, page-numbers:, …)`
4. Emit content nodes (headings, lists, tables, raw, …).

All visual rules (font stack, paragraph spacing, table look, code-block gutter, etc.) live in `src/lib/typst/styles/*` and `admonitions.typ`.

### 4.2 Adding a new style

1. Create `src/lib/typst/styles/<new-style>.typ` exposing `#let article(...) = { ... }`.
2. Extend `src/lib/pipeline/markdownToTypst.ts`:
   - Add the id to the `TypstStyleId` union.
   - Register `{ path, entry }` in `STYLE_TO_TEMPLATE`.
3. Extend `src/lib/workers/typst.worker.ts`:
   - `import xxxTyp from '../typst/styles/<new-style>.typ?raw'`
   - `compiler.addSource('/styles/<new-style>.typ', xxxTyp)` in both init paths.

---

## 5. Fonts & offline guarantee

All fonts are bundled into `static/fonts/` at build time by the `md2pdf-bundle-fonts` Vite plugin (`vite.config.ts`). It downloads them once from upstream on first dev/build, then never again. The Typst worker loads them from `/fonts/*` with `assets: false` passed to `loadFonts()` to suppress typst.ts's default CDN asset bundle.

CJK fonts (Noto Sans CJK SC, Noto Serif SC) and Noto Color Emoji are lazy-loaded only when the worker detects CJK/emoji in the source.

Twemoji SVGs are similarly bundled into `static/twemoji/` (copied from the `twemoji-emojis` npm package by another Vite plugin). The PdfEditor's compile path pre-scans the markdown, fetches the needed SVGs, and injects them into the VFS keyed as `twemoji/<codepoint>.svg`.

The only network calls during a compile are user-supplied remote image URLs (and optionally the user-configured CORS proxy).

---

## 6. Roadmap

- Preflight: typed lint rules + autofixes for common markdown problems (broken tables, code-fence mismatches, very long URLs, lone footnote refs, …).
- Asset manager UI: list, rename, delete IndexedDB-stored images.
- Optional font subsetting to shrink the bundle for mobile.

---

## 7. Testing

No framework yet. The recommended pattern is golden-file regression: drop fixtures into `tests/fixtures/*.md` and assert `compilePdf` returns bytes with empty (or whitelisted) diagnostics.
