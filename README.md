<p align="center">
  <img src="web/static/logo.png" alt="md2pdf logo" width="128" />
</p>

# md2pdf

md2pdf turns Markdown into a typeset PDF or into one self-contained HTML file.
It runs where the document is: in a browser tab or on your own machine. There
is no account, no upload and no server.

You can use it in three ways:

- the web app, an editor with live preview that works offline once loaded
- the command-line tool `bin/md2pdf`, for single files or batches
- a local MCP server, so an AI client can render files in a folder you choose

All three use the same Markdown engine, so a document renders the same way in
each. The full syntax is in [docs/SYNTAX.md](docs/SYNTAX.md).

## What you get

- CommonMark and GFM: tables, task lists, strikethrough, footnotes, autolinks.
- Extras: `==highlight==`, `^sup^`, `~sub~`, `__underline__`, inline
  footnotes, callouts (`:::info`, `:::warning`, …), spoilers, alignment and
  column layouts, `+` column widths in tables, `=WxH` image sizes.
- Math (LaTeX syntax), Mermaid diagrams, Twemoji for emoji and
  `:shortcodes:`.
- YAML frontmatter for the title, authors, date and language, a running
  header and footer, page numbers, a cover page, a DIN 5008 letter layout and
  inline BibTeX citations.
- `[toc]` for a table of contents, `[[pagebreak]]` for a new page, and stable
  heading anchors with `## Heading {#id}`.

The PDF is typeset by [Typst](https://typst.app/). The HTML file embeds its
images, diagrams and math font, so it opens anywhere without a network. It
adapts to screen width, has light and dark themes and a collapsible outline.
Math is MathML, set in New Computer Modern Math, the face the PDF uses. Page
features (cover, letter layout, header, footer, page numbers) have no HTML
counterpart and are skipped there.

## Web app

The preview has two tabs. **Pages** shows the PDF page by page. **Web** shows
the HTML version, which needs no Typst compile and updates as you type.

Documents are saved in the browser (IndexedDB). You can paste or drop images
into the editor, open several tabs (edits sync between them) and export PDF,
HTML or Markdown. Press `?` for the keyboard shortcuts.

Once loaded, the app works offline. The service worker caches about 40 MB,
mostly the Typst compiler. Emoji images and the Chinese, Japanese and Korean
fonts are fetched only when a document needs them. At runtime the app only
contacts the hosts of remote images that a document links to.

The `/reference` page shows the feature demo, [tests/extended.md](tests/extended.md),
with source and result side by side. The same file is the welcome document.

## Command-line tool

`bin/md2pdf` is a Python script (3.9 or newer, standard library only) that runs
on Linux, macOS and Windows. It needs the `typst` binary, version 0.15 or newer
(CI uses 0.15.1), and the md2pdf Typst package installed by `./build.sh
install` (see [Development](#development)).

```
usage: md2pdf [options] input.md [output]
       md2pdf [options] [-o OUTDIR] input.md [input.md ...]
       md2pdf mcp [--root DIR]
```

| Option | Meaning |
|---|---|
| `--pdf` | write PDF (the default) |
| `--html` | write a standalone HTML file |
| `-o`, `--outdir OUTDIR` | directory for the results, created if missing |
| `--timeout SEC` | total time allowed per remote image, default 20 |
| `--max-size MB` | reject a remote image larger than this, default 32 |
| `--allow-private-hosts` | allow remote images on private or link-local addresses |
| `--no-system-fonts` | use only the bundled fonts, so output does not vary by machine |
| `--html-theme {system,light,dark}` | initial theme of the HTML file, default `system` |

```sh
./bin/md2pdf notes.md                        # -> notes.pdf
./bin/md2pdf notes.md out.html               # the extension picks the format
./bin/md2pdf --html notes.md                 # -> notes.html
./bin/md2pdf --html-theme dark notes.md out.html
./bin/md2pdf --html notes.md /dev/stdout     # HTML on stdout, status on stderr
```

With one input, `output` names the file. Its extension picks the format
(`.html` or `.htm` for HTML) unless `--pdf` or `--html` is given. An output
with any other extension is written as PDF. Status lines and warnings go to
stderr.

### Batch mode

Pass several files, or use `-o`, to render a batch:

```sh
./bin/md2pdf chapter1.md chapter2.md chapter3.md
./bin/md2pdf --pdf --html -o out/ docs/*.md
```

Each input becomes `<name>.pdf` and/or `<name>.html` in OUTDIR, or next to the
input when there is no `-o`. `--pdf --html` writes both. If a file fails, md2pdf
reports it and renders the rest. The exit status is 1 if any file failed.

Two Markdown files (`.md`, `.markdown`, `.mdown`, `.mkd`) are a batch of two,
not an input and an output.

### Remote images

md2pdf downloads remote images fresh on every run into a hidden folder next to
the document, `.md2pdf-<id>-remote`, and deletes it afterwards. Nothing is
cached between runs, so one document cannot read another's downloads.

Downloads are limited to public `http(s)` hosts, 32 MB and 20 seconds per
image. `--allow-private-hosts`, `--max-size` and `--timeout` change these
limits. A URL that fails, or that returns an HTML page instead of an image,
becomes a blank placeholder. A local image that is missing produces a warning
and a placeholder. In both cases the rest of the document still renders.

The web app applies the same 32 MB and 20 second limits. It also fetches at
most 64 remote images per document, 6 at a time, and never sends credentials.

## MCP server

```sh
./bin/md2pdf mcp --root /path/to/workspace
```

This starts a local MCP server over stdio. Configure your client with the
absolute path to `bin/md2pdf` as the command and `mcp --root <workspace>` as
the arguments. The server offers:

- the tool `render_file`, which renders a Markdown file to PDF or HTML.
  Input and output paths must be inside the root.
- the resource `md2pdf://syntax`, which is [docs/SYNTAX.md](docs/SYNTAX.md)
- the resource `md2pdf://example`, which is [tests/extended.md](tests/extended.md)

## HedgeDoc and Outline

Markdown copied from HedgeDoc or from Outline ("Copy as Markdown") renders
without changes. That covers HedgeDoc's `++inserted++`, `:::spoiler`, bylines,
`{%youtube …%}` style embeds (as links), `=WxH` image
sizes and `js=101` fence flags, and Outline's notices, nested `+++` toggles,
empty `\` paragraphs and captioned image sizes. The details are in
[docs/SYNTAX.md](docs/SYNTAX.md#hedgedoc-and-outline).

## Limits

- Raw HTML is shown as text, except a short list of formatting tags such as
  `<b>`, `<kbd>`, `<sup>` and `<br>`.
- Only `mermaid` fences become diagrams. Other diagram languages are shown as
  code.
- Embeds (`{%youtube …%}` and others) become links, not players.
- The HTML file uses the reader's system fonts, so it does not look exactly
  like the PDF. The content is the same.
- The PDF uses A4 paper.
- In the web app, a remote image whose host does not allow cross-origin
  requests (CORS) becomes a placeholder.

## Scripts other than Latin

Both outputs are UTF-8 throughout, and heading anchors keep Unicode letters,
so `# 你好` gets a working link. Fonts are where a script can go missing, and
each front-end handles this differently:

- HTML uses the reader's system fonts, so their system supplies the glyphs.
- The web app's PDF and Pages preview use only the fonts it hands Typst, and
  the bundled ones are Latin. When a document contains Chinese, Japanese or
  Korean, the app fetches Noto Sans SC or KR (about 8 MB per weight) once per
  session.
- The CLI uses installed system fonts, so whatever CJK font the machine has is
  used. `--no-system-fonts` turns this off for reproducible output, at the cost
  of missing glyphs.

`tests/unicode.md` covers Chinese, Japanese, Korean, Arabic, combining marks
and CJK headings.

## How it works

The Markdown engine is a Rust crate built on
[`comrak`](https://github.com/kivikakk/comrak). It is compiled to WebAssembly,
loaded as a Typst plugin and shipped in a Typst package (`@local/md2pdf`). The
web app and the CLI both hand raw Markdown to this engine.

```
                         ┌─ Typst markup ─▶ Typst compile ─▶ PDF
Markdown ─▶ engine.wasm ─┤                 (typst / typst.ts)
                         └─ HTML ─────────▶ one self-contained .html file
```

The engine parses once and has two renderers. The PDF path produces Typst
markup. The HTML path produces the finished file. The CLI uses the `typst`
binary (0.15.1 in CI). The web app runs Typst 0.15.1 in the browser through
typst.ts 0.8.0-rc4.

Design notes are in [docs/DESIGN.md](docs/DESIGN.md), engineering notes in
[docs/ENG.md](docs/ENG.md).

## Repository layout

| Path | What it is |
|---|---|
| `web/` | The SvelteKit web app, deployed to GitLab Pages. See [web/README.md](web/README.md). |
| `engine/` | The Rust crate that builds `engine.wasm`. |
| `package/` | The `md2pdf` Typst package: `lib.typ`, styles, cover, header and footer, callouts, vendored `mitex` and `mmdr`, Twemoji SVGs and the built `engine.wasm`. |
| `bin/md2pdf` | The CLI and MCP server. It fetches remote images and runs Typst. |
| `fonts/` | Fonts shared by the CLI and the web app. |
| `build.sh` | Builds `engine.wasm` and installs the package as `@local/md2pdf`. |
| `tests/` | Markdown fixtures, CLI tests, HTML checks and render goldens. |
| `docs/` | Syntax reference, design and engineering notes. |

## Development

```sh
./build.sh                  # build engine.wasm and install @local/md2pdf
./build.sh engine           # only rebuild package/engine.wasm
./build.sh install          # only install package/ as @local/md2pdf
cd engine && cargo test     # engine tests
python3 -m unittest discover -s tests -p "test_*.py"   # CLI tests
python3 tests/visual.py     # compare renders with tests/golden/
python3 tests/visual.py --update                        # rewrite the goldens
python3 tests/check_html.py out.html                    # check an HTML export
```

Building the engine needs Rust. The toolchain is pinned in
`engine/rust-toolchain.toml`, including the `wasm32-unknown-unknown` target,
and `rustup` installs it on first use. `tests/visual.py` needs `typst`,
`pdftoppm` (poppler) and Pillow.

`package/engine.wasm` is committed, so the web build needs no Rust. After any
change in `engine/`, run `./build.sh engine` and commit the new
`engine.wasm`. The build is reproducible, and CI fails if the committed file
does not match the source.

For the web app:

```sh
cd web
npm install
npm run dev          # local dev server
npm test             # unit tests
npm run test:e2e     # browser tests (Playwright)
npm run build        # static build -> web/build/
```

The build reads `PUBLIC_SITE_URL` (the absolute base URL for social-preview
tags) at build time through `web/src/env.ts`. More in
[web/README.md](web/README.md).

`tests/check_html.py` checks an exported HTML file: WCAG AA contrast for every
colour token, syntax colours included, in both themes; an `alt` on every image;
every in-page link resolves; no external resource, no remote `url()`, no inline
event handler and no `javascript:` or `data:` link. The only web font it allows
is the math face, embedded as a `data:` URI.

CI (GitHub Actions) runs five jobs: `engine` (format, clippy, tests and the
`engine.wasm` check), `cli`, `render` (the goldens), `web-lint` (Prettier,
svelte-check, unit tests) and `web-e2e`.

Fixtures worth knowing: `tests/extended.md` is the feature demo and documents
every frontmatter key. `tests/html-edge.md` holds hostile input for the HTML
renderer. `tests/hedgedoc.md` and `tests/outline.md` cover the imported
syntax, and `tests/regressions.md` collects inputs that once broke a render.

## License

[MIT](LICENSE)
