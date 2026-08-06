<p align="center">
  <img src="web/static/logo.png" alt="md2pdf logo" width="128" />
</p>

# md2pdf

**Markdown → PDF or HTML with perfect typesetting — all Markdown processing
lives in one engine.**

The Markdown engine is a Rust/[`comrak`](https://github.com/kivikakk/comrak)
parser compiled to a WebAssembly [Typst](https://typst.app/) plugin and shipped
as a Typst package (`@local/md2pdf`). Both front-ends — the browser app and the
command-line tool — feed raw Markdown to the *same* engine, so output is
identical.

```
                        ┌─ Typst markup ─▶ Typst compile ─▶ PDF
Markdown ─▶ engine.wasm ─┤                 └── typst / typst.ts ──┘
                        └─ HTML ─────────▶ one self-contained .html file
```

The engine has two renderers over one parse. PDF goes through Typst for
typesetting; HTML comes straight out of the engine, styled, self-contained
(images and diagrams embedded as `data:` URIs) and responsive, in light and
dark. Page-only features — cover page, DIN letter mode, running header/footer,
page numbers — have no HTML counterpart and are skipped there. HTML also gets a
collapsible outline, hidden behind a button in the corner, and real math: the
MathML is laid out with New Computer Modern Math, the same face the PDF uses,
embedded in the file as a subsetted `woff2`.

## Repository layout

| Path         | What it is |
|--------------|------------|
| `web/`       | The SvelteKit web app (live editor + preview). Deployed to GitLab Pages. |
| `engine/`    | Rust crate — builds `engine.wasm`, the Markdown engine. |
| `package/`   | The `md2pdf` Typst package: `lib.typ`, `styles/`, `admonitions.typ`, vendored `mitex`/`mmdr`, bundled Twemoji SVGs, and the built `engine.wasm`. |
| `bin/md2pdf`  | CLI host shim — discovers remote images, fetches them, then runs Typst. Pure stdlib Python. |
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
npm test             # unit tests
npm run build        # static build → web/build/
```

The preview pane has two tabs: **Pages** is the paged SVG preview of the PDF,
**Web** is the pageless HTML view. The HTML view needs no Typst compile,
so it updates as you type.

The `web/vite.config.ts` plugins copy `package/` and `fonts/` into
`web/static/` at build time — the app is fully offline, no CDN calls.

## CLI

`bin/md2pdf` runs on Windows, macOS and Linux with a stock Python 3.9+ and
no third-party packages. It needs the `typst` binary (v0.15+, for `typst eval`)
and, to rebuild the engine, Rust with the `wasm32-unknown-unknown` target
(`rustup target add wasm32-unknown-unknown`, or the `rust-wasm` package on Arch).

```sh
./build.sh                                     # build engine.wasm + install @local/md2pdf
python3 bin/md2pdf tests/sample.md             # → tests/sample.pdf
python3 bin/md2pdf tests/sample.md out.html    # → HTML (the extension picks the format)
./bin/md2pdf --html tests/sample.md            # → tests/sample.html
cd engine && cargo test                        # engine unit tests
python3 tests/check_html.py out.html           # assert the rendered artefact
```

`tests/check_html.py` checks the file that actually ships: WCAG AA contrast for
every colour token *including the syntax-highlighting ones*, in both themes; an
`alt` on every image; every in-page link resolving; no external resource, no
remote `url()`, no inline event handler and no `javascript:`/`data:` anchor.
The one webfont it allows is the math face, and only as a `data:` URI.

Remote images are fetched fresh each run — nothing is cached between runs, so
no document can read another's downloads. The fetch is limited to public
http(s) hosts, capped at 32 MB and 20 s per image; `--allow-private-hosts`,
`--max-size` and `--timeout` adjust that. A URL that cannot be fetched becomes
a blank placeholder rather than failing the render.

The browser applies the same 32 MB and 20 s ceilings, and additionally caps a
document at 64 remote images fetched 6 at a time, so a long image list cannot
spend the tab's memory and connections on the document author's behalf. Those
requests never carry credentials.

## Markdown coverage

Core CommonMark + GFM (tables incl. `+` column-width markers, task lists,
strikethrough, footnotes, autolinks); `==highlight==`, super/subscript,
`__underline__`; language-aware admonitions (`:::info` …, including `caution` and
`important`), spoilers (`+++++`), `:::row/center`
layout; math via `mitex`; Mermaid via `mmdr`; HackMD `=WxH` image sizing;
remote images; Twemoji emoji (unicode and `:shortcodes:`); YAML frontmatter
(title / authors / date / `lang` / page numbers); running header and footer
(`header-*` / `footer-*`, optional `header-height` / `footer-height`, and
`{page}`-style placeholders); optional title
cover page (`cover-*`); opt-in inline BibTeX citations (`bibliography: inline`,
`[@key]`, blue citation numerals, and `bibliography-style`, default `ieee`);
DIN 5008 letter mode (`letter-*` fields);
`[toc]` → `#outline()`; `hide-toc-button: true` to drop the HTML outline
drawer without touching an explicit `[toc]`.

`<br>` is the only raw HTML tag either renderer honours — it is the one way to
break a line inside a table cell. Everything else is shown as text.

## Non-Latin scripts

Both renderers are UTF-8 end to end, and heading slugs are built from Unicode
alphanumerics, so `# 你好` gets a real anchor. Type is the only place a script
can go missing, and the two front-ends solve it differently:

- **HTML** — nothing to solve. The sheet asks for the system UI stack, and the
  reader's own system supplies the face.
- **Web app, paged preview and PDF export** — Typst only sees the faces we hand
  it, and `fonts/` is Latin. The app therefore detects CJK, kana and Hangul in
  the source and fetches Noto Sans SC or KR (~8 MB per weight, from
  `web/static/fonts/`, populated at build time) once per session, only for
  documents that contain those scripts.
- **CLI** — installed system fonts are used, so whatever the machine has for
  CJK is picked up. `--no-system-fonts` restores the hermetic behaviour for
  reproducible builds, at the cost of tofu where the bundled fonts have no
  glyph.

`tests/unicode.md` is the fixture: Chinese, Japanese, Korean, Arabic RTL,
combining marks, wide characters inside a code fence, and CJK headings.

Everything above renders in both targets. `tests/html-edge.md` is the
adversarial fixture for the HTML renderer (injection attempts, broken input,
structural extremes).

`tests/extended.md` is the feature demo — it exercises every syntax above and
documents every frontmatter key, header/footer placeholder and cover option.
It is also the web app's welcome document (`web/src/lib/templates/pdf-templates.ts`
holds a copy that must stay in sync).

## License

[MIT](LICENSE)
