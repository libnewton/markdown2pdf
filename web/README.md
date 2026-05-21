<p align="center">
  <img src="static/logo.png" alt="md2pdf logo" width="128" />
</p>

# md2pdf — web app

**Markdown to PDF, perfect typesetting, fully in the browser.**

A Markdown export tool built with [Svelte 5](https://svelte.dev/) +
[SvelteKit](https://kit.svelte.dev) and [Typst](https://typst.app/). Everything
runs client-side via WebAssembly — no server, no setup, no telemetry.

> This is the web front-end of the [md2pdf monorepo](../README.md). Markdown
> processing lives in the shared `md2pdf` Typst package (`../package/`), not in
> this app — see the root README for the architecture.
>
> Based on [cosformula/mdxport](https://github.com/cosformula/mdxport).

## Features

- **Live preview** — SVG preview rendered directly from Typst as you type, with
  a pause toggle (`Ctrl/Cmd+Enter` to compile on demand).
- **Code editor** — CodeMirror 6 with Markdown syntax highlighting.
- **Document management** — auto-saves to IndexedDB; switch between recent docs.
- **Image upload** — paste or drop images straight into the editor.
- **Page breaks** — `[[pagebreak]]` for manual pagination.
- **Offline by default** — fonts and the Typst package are bundled into
  `static/` at build time; the only network call during a compile is for
  user-supplied remote image URLs.
- **PWA** — installable, works offline after first load.

See the [root README](../README.md) for full Markdown coverage.

## Development

```bash
npm install
npm run dev          # dev server
npm run build        # static build → build/
npm run check        # type-check
npm test             # unit tests
```

The `vite.config.ts` plugins copy the `md2pdf` Typst package (`../package/`)
and shared fonts (`../fonts/`) into `static/` on every build.

## Tech stack

- [Svelte 5](https://svelte.dev/) + [SvelteKit](https://kit.svelte.dev)
  (`adapter-static`)
- [Typst](https://typst.app/) via [typst.ts](https://github.com/Myriad-Dreamin/typst.ts)
  (compiler + SVG renderer, in a Web Worker)
- [CodeMirror 6](https://codemirror.net/) editor
- The shared Rust/WASM Markdown engine (`../engine/`)

## License

[MIT](../LICENSE)
