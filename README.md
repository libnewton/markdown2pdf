<p align="center">
  <img src="web/static/logo.png" alt="md2pdf logo" width="128" />
</p>

# md2pdf

**Markdown → PDF with perfect typesetting — all Markdown processing lives
inside Typst.**

The Markdown engine is a Rust/[`comrak`](https://github.com/kivikakk/comrak)
parser compiled to a WebAssembly [Typst](https://typst.app/) plugin and shipped
as a Typst package (`@local/md2pdf`). Both front-ends — the browser app and the
command-line tool — feed raw Markdown to the *same* engine, so output is
identical.

```
Markdown ─▶ engine.wasm (Rust/comrak) ─▶ Typst markup ─▶ Typst compile ─▶ PDF
           └──── inside the Typst package ────┘          └── typst / typst.ts ──┘
```

## Repository layout

| Path         | What it is |
|--------------|------------|
| `web/`       | The SvelteKit web app (live editor + preview). Deployed to GitLab Pages. |
| `engine/`    | Rust crate — builds `engine.wasm`, the Markdown engine. |
| `package/`   | The `md2pdf` Typst package: `lib.typ`, `styles/`, `admonitions.typ`, vendored `mitex`/`mmdr`, bundled Twemoji SVGs, and the built `engine.wasm`. |
| `bin/md2pdf` | CLI host shim — discovers remote images, then runs `typst compile`. |
| `fonts/`     | Fonts shared by the CLI and the web app. |
| `build.sh`   | Builds `engine.wasm` and installs the package to `@local/md2pdf`. |
| `tests/`     | Markdown fixtures. |

`package/engine.wasm` is committed (so the web build needs no Rust toolchain);
rerun `./build.sh` after changing anything in `engine/`.

## Web app

```sh
cd web
npm install
npm run dev          # local dev server
npm run build        # static build → web/build/
```

The `web/vite.config.ts` plugins copy `package/` and `fonts/` into
`web/static/` at build time — the app is fully offline, no CDN calls.

## CLI

Requires the `typst` binary (v0.13+) and, to rebuild the engine, Rust with the
`wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`, or
the `rust-wasm` package on Arch).

```sh
./build.sh                       # build engine.wasm + install @local/md2pdf
./bin/md2pdf tests/sample.md     # → tests/sample.pdf
```

## Markdown coverage

Core CommonMark + GFM (tables incl. `+` column-width markers, task lists,
strikethrough, footnotes, autolinks); `==highlight==`, super/subscript,
underline; admonitions (`:::info` …), spoilers (`+++++`), `:::row/center`
layout; math via `mitex`; Mermaid via `mmdr`; HackMD `=WxH` image sizing;
remote images; Twemoji emoji (unicode and `:shortcodes:`); YAML frontmatter
(title / authors / date / `lang` / page numbers); running header and footer
(`header-*` / `footer-*`, with `{page}`-style placeholders); optional title
cover page (`cover-*`); DIN 5008 letter mode (`letter-*` fields);
`[toc]` → `#outline()`.

## License

[MIT](LICENSE)
