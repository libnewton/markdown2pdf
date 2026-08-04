# Repository Guidelines

md2pdf is a Markdown → PDF *and* HTML tool. **All Markdown processing lives in
a Rust/WASM engine inside a Typst package** — both front-ends (the web app and
the CLI) feed raw Markdown to that one engine.

The engine has two renderers over one comrak parse: `engine/src/lib.rs` emits
Typst markup for the PDF, and `engine/src/html/` emits HTML. The HTML target
never runs Typst — the browser calls `engine.wasm` directly, and the CLI reads
the result out of a `typst query`. Whatever the engine cannot do itself (read
image bytes, run the Mermaid plugin) each host resolves and hands back as one
blob plus a `key<TAB>byte-length` manifest.

## Repository layout

This is a monorepo:

- `web/`: the SvelteKit web app (live editor + preview). Deployed to GitLab
  Pages via `.gitlab-ci.yml`.
  - `web/src/routes/+page.svelte`: the app's only page (the PDF editor).
  - `web/src/lib/workers/typst.worker.ts`: runs the Typst compile (and the
    engine WASM) in a Web Worker. The worker feeds raw Markdown to Typst.
  - `web/src/lib/components/`: editor + preview UI.
  - `web/vite.config.ts`: build-time plugins copy `package/` → `static/md2pdf/`
    and `fonts/` → `static/fonts/`.
- `engine/`: Rust crate (`comrak`-based) — builds `engine.wasm`, the Markdown
  engine. It is a Typst WASM plugin.
  - `engine/src/html/`: the HTML renderer — stylesheet, outline, LaTeX→MathML
    (`math-core`), syntax highlighting, `data:` URI embedding.
- `package/`: the `md2pdf` Typst package — `lib.typ`, `styles/modern-tech.typ`,
  `admonitions.typ`, vendored `mitex`/`mmdr`, bundled Twemoji SVGs, and the
  built `engine.wasm`.
- `bin/md2pdf`: CLI host shim (remote-image prefetch + `typst compile`).
- `fonts/`: Typst fonts shared by the CLI and the web app.
- `build.sh`: builds `engine.wasm` and installs the package to `@local/md2pdf`.
- `tests/`: Markdown fixtures for the CLI.
- `docs/`: design and engineering notes.

`package/engine.wasm` is committed (the web build needs no Rust toolchain).
Rerun `./build.sh` after changing anything in `engine/`.

## Build, test, dev commands

- Engine + package: `./build.sh` (needs Rust + `wasm32-unknown-unknown`).
- Engine tests: `cd engine && cargo test`.
- CLI: `./bin/md2pdf tests/sample.md` (needs the `typst` binary);
  `./bin/md2pdf --html tests/sample.md` for the HTML target.
- Web app — run inside `web/`:
  - Install: `npm install`
  - Dev server: `npm run dev`
  - Type check: `npm run check`
  - Unit tests: `npm test` (Vitest)
  - Production build (static): `npm run build`

## Code style

- TypeScript/Svelte: 2-space indent, `camelCase`; files named by responsibility.
- Rust (`engine/`): standard `rustfmt`.
- **Templates own styling.** The engine emits content-only Typst markup; never
  hardcode `set`/`show` rules in the engine. Typesetting lives in
  `package/styles/*.typ` and `package/admonitions.typ`. The HTML target is the
  exception and owns its own stylesheet (`engine/src/html/css.rs`), because
  Typst is not involved in it at all — keep the two palettes in step by hand.
- **Browser boundary**: page code must guard browser APIs with
  `$app/environment`'s `browser` or `onMount()`. Typst/WASM init belongs in the
  Web Worker.

## Commits & PRs

- **Conventional Commits** (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:` …).
- PRs should describe motivation, scope, and user-visible behaviour. For
  UI/typesetting changes, attach a screenshot or an exported PDF sample.

## Troubleshooting

- Typst errors cite a `.typ` file + line:col. The worker compiles a generated
  `/main.typ` that imports `package/lib.typ`; the engine output is `eval`'d.
- For raw-block errors like `unclosed raw text`, the offending Markdown almost
  always has a missing or mismatched code fence.

## Architectural constraints (hard rules)

md2pdf ships as a **static-deployed, fully client-side pipeline**:

- No SvelteKit server features (no `+server.ts`, no form actions).
- No browser API access at module top level; Workers/WASM must initialize
  inside the client lifecycle.
- **Offline by default.** All fonts and Twemoji SVGs are bundled into
  `web/static/` at build time — no CDN calls. The only network call is the
  explicit remote-image URL feature (user-supplied URLs in Markdown), which is
  also the only thing the optional CORS-proxy setting affects.
- One Markdown codebase: the Rust/WASM engine. Do not reintroduce
  Markdown parsing in TypeScript.
- **Both renderers or neither.** A new syntax feature has to land in the Typst
  renderer *and* the HTML one; `every_construct_renders_in_both_outputs` in
  `engine/src/html/tests.rs` fails otherwise.
