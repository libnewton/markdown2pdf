# Repository Guidelines

## Project layout

- `src/routes/`: SvelteKit pages and layouts.
  - `src/routes/+page.svelte`: PDF editor (the app's only entry).
  - `src/routes/cards/+page.svelte`: Cards mode.
  - `src/routes/slides/+page.svelte`: Slides mode.
- `src/lib/pipeline/`: Markdown → Typst conversion. Core: `src/lib/pipeline/markdownToTypst.ts`. Custom remark plugins live in `src/lib/pipeline/plugins/`.
- `src/lib/typst/`: Typst typesetting templates (styling decoupled from content).
  - `src/lib/typst/styles/*.typ`: One file per style (only `modern-tech` for PDF; redbook variants for cards; slides-* for slides).
  - `src/lib/typst/admonitions.typ`: Shared admonition/spoiler/task-item/list-marker helpers, imported by every style.
- `src/lib/workers/`: WASM/CPU-heavy work (Typst compile) runs in a Web Worker to keep the UI thread free.
- `src/lib/twemoji/loader.ts` + `static/twemoji/`: Twemoji SVG asset pipeline.
- `static/fonts/`: All Typst fonts, bundled at build time by the `md2pdf-bundle-fonts` Vite plugin in `vite.config.ts`.
- `docs/`: Design and engineering notes (`DESIGN.md`, `ENG.md`, `MVP_PLAN.md`).

## Build, test, dev commands

- Install: `npm install`
- Dev server: `npm run dev`
- Type check: `npm run check`
- Production build (static): `npm run build`
- Preview built output: `npm run preview`

## Code style

- Indent: 2 spaces in TypeScript/Svelte; no tabs.
- Naming: `camelCase` for TS functions/variables; files named by responsibility (e.g. `typstClient.ts`, `renderer.ts`).
- **Templates own styling.** Keep `markdownToTypst.ts` purely about content emission; never hardcode `set/show` rules in the generator. Tweak typesetting in `src/lib/typst/styles/*` or `admonitions.typ`. Add new styles via the `MarkdownToTypstOptions.style` enum.
- **Browser boundary**: page code must guard browser APIs with `$app/environment`'s `browser` or `onMount()`. Typst/WASM initialization belongs in `src/lib/workers/*`.

## Testing

No test framework is wired up. For regression coverage, drop `.md` fixtures into `tests/fixtures/` and assert the worker can compile them without diagnostics.

## Commits & PRs

- **Conventional Commits** (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:` …).
- PRs should describe motivation, scope, and user-visible behaviour. For UI/typesetting changes, attach a screenshot or an exported PDF sample.

## Troubleshooting

- Typst errors usually cite `main.typ:line:col`. The full generated Typst is held in the editor — log `lastCompiledTypst` from `PdfEditor.svelte` to inspect it.
- For raw-block errors like `unclosed raw text`, the offending markdown almost always has a missing or mismatched code fence.

## Architectural constraints (hard rules)

md2pdf ships as a **static-deployed, fully client-side pipeline**:

- No SvelteKit server features (no `+server.ts`, no form actions).
- No browser API access at module top level; Workers/WASM must initialize inside the client lifecycle (e.g. `onMount`).
- **Offline by default.** All fonts and twemoji SVGs are bundled into `static/`. The only network call is the explicit remote-image URL feature (user-supplied URLs in markdown), which is also the only thing the optional CORS-proxy setting affects.
