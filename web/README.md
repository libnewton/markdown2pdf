<p align="center">
  <img src="static/logo.png" alt="md2pdf logo" width="128" />
</p>

# md2pdf web app

A Markdown editor that exports a typeset PDF or a self-contained HTML file,
entirely in the browser. It is built with [Svelte 5](https://svelte.dev/),
[SvelteKit](https://svelte.dev/docs/kit) and [Typst](https://typst.app/).
Everything runs on the client through WebAssembly: no server, no setup, no
telemetry.

> This is the web front-end of the [md2pdf repository](../README.md). Markdown
> processing lives in the shared engine and Typst package (`../engine/`,
> `../package/`), not in this app. See the root README for the architecture.
>
> Based on [cosformula/mdxport](https://github.com/cosformula/mdxport).

## Features

- **Live preview.** The **Web** tab shows the HTML version and needs no Typst
  compile, so it updates as you type. The **Pages** tab shows the PDF page by
  page and compiles only while it is on screen.
- **Editor.** CodeMirror 6 with Markdown highlighting. Slash commands (`/new`,
  `/toc`, `/table`, …) expand on Enter when they are alone on a line. Press
  `?` for the keyboard shortcuts.
- **Documents.** Autosaved to IndexedDB. The document menu lists all local
  documents and deletes with confirmation. Edits sync live between open tabs.
- **Images.** Paste or drop images into the editor. Remote images are fetched
  by the browser, and a host that blocks cross-origin requests gives a
  placeholder.
- **Export.** PDF, standalone HTML (opening in the app's current theme) and
  Markdown.
- **Offline.** Installable as a PWA. After the first visit it works offline:
  the service worker caches about 40 MB, mostly the Typst compiler. Emoji and
  CJK fonts are fetched only for documents that use them.
- **Reference.** `/reference` shows the feature demo with source and result
  side by side, without touching your documents.

The full syntax is in [docs/SYNTAX.md](../docs/SYNTAX.md).

## Development

```bash
npm install
npm run dev            # dev server
npm run build          # static build -> build/
npm run preview        # serve the build
npm run check          # svelte-check, expected at 0 errors and 0 warnings
npm test               # unit tests (vitest)
npm run test:e2e       # browser tests (Playwright)
npm run format         # Prettier, rewrite files
npm run format:check   # Prettier, check only (CI runs this)
```

`npm run test:e2e` runs against a production build, so run `npm run build`
first.

SvelteKit 3 has no `svelte.config.js`. The Kit, adapter and PWA settings live
in `vite.config.ts`. Its plugins also copy the `md2pdf` Typst package
(`../package/`) and the shared fonts (`../fonts/`) into `static/` on every
build. The Noto Sans SC/KR fonts are downloaded once into `.cache/cjk` and
copied from there.

App modules are imported as `#lib/…`, through the `imports` map in
`package.json`.

`PUBLIC_SITE_URL` sets the absolute base URL for social-preview meta tags. It
is read at build time through `src/env.ts`.

The welcome document is `../tests/extended.md`, imported directly. Edit that
file, not a copy.

Engineering notes for the app (worker protocol, previews, storage, tests) are
in [docs/ENG.md](../docs/ENG.md).

## Tech stack

- [Svelte 5](https://svelte.dev/) and [SvelteKit](https://svelte.dev/docs/kit)
  with `adapter-static`
- [Typst](https://typst.app/) 0.15.1 through
  [typst.ts](https://github.com/Myriad-Dreamin/typst.ts) 0.8.0-rc4 (compiler and
  SVG renderer, in a Web Worker)
- [CodeMirror 6](https://codemirror.net/)
- The shared Rust/WebAssembly Markdown engine (`../engine/`)

## License

[MIT](../LICENSE)
