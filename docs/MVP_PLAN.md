# md2pdf — MVP status (2026-05-16)

## Goal & scope

**Goal**: in a fully-static front-end (no backend), close the loop "Markdown in → typeset PDF preview → PDF download".

**Engineering constraints** (see `docs/ENG.md`):

- All routes are statically prerendered (SvelteKit + adapter-static).
- No `+server.ts`, no form actions.
- Typst/WASM initialized in the browser, inside a Web Worker.

---

## Done (v0.0.1)

### Core pipeline

- Markdown (mdast) → Typst: `src/lib/pipeline/markdownToTypst.ts`
- Typst (WASM) compile in a Worker: `src/lib/workers/typst.worker.ts`
- Main-thread wrapper: `src/lib/workers/typstClient.ts`
- SVG preview (typst.ts renderer): `src/lib/typst/renderer.ts` + `src/lib/typst/svg-utils.ts`

### Routes & static deployment

- Global `prerender = true` + `trailingSlash = 'always'`: `src/routes/+layout.ts`.
- Three routes — `/`, `/cards/`, `/slides/`. No dynamic segments, no `EntryGenerator` needed.

### Markdown coverage

Covers everything in the feature-demo template (`src/lib/templates/pdf-templates.ts`, `WELCOME`):

- Headings, paragraphs, bold / italic / strikethrough, links (inline + reference)
- Lists (nested), ordered + unordered + task lists, with cycling markers
- Inline code, fenced code blocks with line numbers, GFM tables with rounded borders
- Math (`$…$` and `$$…$$`)
- `==highlight==`, super- and subscript, footnotes
- Themed admonitions (`:::success`, `:::warning`, `:::tip`, `:::info`, `:::danger`)
- Spoiler blocks (`+++++ … +++++`)
- Images: local, remote (with optional CORS proxy), HackMD-style `=WxH` sizing, alt-as-caption
- Emojis: unicode (via Twemoji SVGs) and `:shortcode:` (via node-emoji)
- `[toc]` → `#outline()`
- Mermaid: ```mermaid → SVG → injected into Typst VFS as an image
- `[[pagebreak]]` token

### Offline guarantee

- All fonts and twemoji SVGs are bundled into `static/` at build time (`vite.config.ts` plugins).
- typst.ts's default jsdelivr font assets are explicitly suppressed via `loadFonts(urls, { assets: false })`.
- Analytics has been removed (`src/hooks.client.ts` is empty).

---

## Next (proposed v0.0.2 / v0.1)

By risk:

1. **Preflight (diagnostics + fixes)** — turn "compile-failing or layout-breaking" patterns into explainable, one-click-fixable rules (broken tables, code-fence mismatches, super-long URLs, lone footnote refs, …).
2. **Asset manager UI** — list/rename/delete IndexedDB-stored images (currently only addable via paste/drop).
3. **Per-template configuration in the editor** — expose font/density/theme switches instead of only style id.
4. **Mobile polish** — sizing of code line numbers, table overflow handling.

---

## Regression testing (suggested)

No test framework is wired up. The recommended approach is golden-file regression:

- Drop `.md` fixtures into `tests/fixtures/` covering tables, long bodies, math, emoji, mermaid, very long URLs/tokens, and image variants.
- Assert: `compilePdf` returns non-empty bytes, `diagnostics` is either empty or in an allowed-list, page count + byte size within bounds.
