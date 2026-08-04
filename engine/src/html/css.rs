//! The document stylesheet, inlined into every rendered document.
//!
//! Written once and shared by the standalone export and the editor's preview
//! pane, which mounts the same markup in a shadow root — hence `:root, :host`
//! on the token block, so the variables land either way.
//!
//! Colours track `styles/modern-tech.typ` and `admonitions.typ` so a document
//! reads the same in both outputs. Every colour is a variable with a dark
//! counterpart; nothing else in the sheet hardcodes one.

pub(crate) const STYLE: &str = r#"
/* One token block. `light-dark()` picks a side from the element's
   `color-scheme`, so the OS preference and the explicit `data-theme` override
   share a single set of definitions instead of a light copy and a dark copy. */
:root, :host { color-scheme: light dark; }
:root, :host, .md2pdf {
  --md-bg: light-dark(#ffffff, #14171c);
  --md-fg: light-dark(#16191d, #dde2e9);
  --md-heading: light-dark(#333333, #eef1f5);
  --md-muted: light-dark(#6b7280, #9aa4b2);
  --md-accent: light-dark(#0074de, #62b0ff);
  --md-rule: light-dark(#e3e6ea, #2b313a);
  --md-surface: light-dark(#f6f7f9, #1b1f26);
  --md-surface-2: light-dark(#eef0f3, #232830);
  --md-mark-bg: light-dark(#fef08a, #6b5a12);
  --md-mark-fg: light-dark(#453c05, #fdf6d8);
  --md-quote-bg: light-dark(#f8f9fa, #1a1e25);
  --md-scrim: light-dark(rgba(16, 24, 40, .3), rgba(0, 0, 0, .55));
  --md-shadow-color: light-dark(rgba(16, 24, 40, .12), rgba(0, 0, 0, .55));
  --md-shadow: 0 1px 2px var(--md-shadow-color), 0 8px 24px var(--md-shadow-color);
  --md-measure: 46rem;
  --md-t-c: light-dark(#7b8494, #7f8a9c);
  --md-t-s: light-dark(#0a7d55, #6dd3a4);
  --md-t-n: light-dark(#b45309, #f0b464);
  --md-t-k: light-dark(#9333ea, #c79bf5);
  --md-t-t: light-dark(#0369a1, #6cc4f0);
  --md-t-m: light-dark(#be185d, #f58bb4);
  --md-ok: light-dark(#16a34a, #4ade80);
  --md-adm-success: light-dark(#16a34a, #4ade80);
  --md-adm-warning: light-dark(#d97706, #f5b544);
  --md-adm-tip: light-dark(#0ea5e9, #56c6f5);
  --md-adm-info: light-dark(#2563eb, #7aa6ff);
  --md-adm-danger: light-dark(#dc2626, #ff8080);
  --md-adm-note: light-dark(#6b7280, #a3adba);
  --md-adm-caution: light-dark(#b91c1c, #ff8f8f);
  --md-adm-important: light-dark(#7e22ce, #cd96f7);
  --md-adm-success-bg: light-dark(#f0fdf4, #10241a);
  --md-adm-warning-bg: light-dark(#fffbeb, #2a1f08);
  --md-adm-tip-bg: light-dark(#f0f9ff, #0b2130);
  --md-adm-info-bg: light-dark(#eff6ff, #111e35);
  --md-adm-danger-bg: light-dark(#fef2f2, #2b1414);
  --md-adm-note-bg: light-dark(#f9fafb, #1b1f26);
  --md-adm-caution-bg: light-dark(#fef2f2, #2b1414);
  --md-adm-important-bg: light-dark(#faf5ff, #21132f);
}
/* An explicit override may sit on the fragment root, the document root, or
   the shadow host the editor mounts it in — all three must win. */
[data-theme="light"], :host([data-theme="light"]) { color-scheme: light; }
[data-theme="dark"], :host([data-theme="dark"]) { color-scheme: dark; }

/* ---- shell ------------------------------------------------------------ */

.md2pdf {
  background: var(--md-bg);
  color: var(--md-fg);
  font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto,
    "IBM Plex Sans", "Helvetica Neue", Arial, sans-serif;
  font-size: 17px;
  line-height: 1.65;
  text-rendering: optimizeLegibility;
  -webkit-text-size-adjust: 100%;
  min-height: 100dvh;
  position: relative;
  overflow-wrap: break-word;
}
.md2pdf *, .md2pdf *::before, .md2pdf *::after { box-sizing: border-box; }
.md2pdf-doc {
  max-width: var(--md-measure);
  margin: 0 auto;
  padding: 3.5rem 1.5rem 8rem;
}
@media (max-width: 640px) {
  .md2pdf { font-size: 16px; }
  .md2pdf-doc { padding: 2rem 1.1rem 5rem; }
}
/* Room for the outline button above the title. `:not(:first-child)` means only
   documents that actually have an outline reserve it. The button is positioned
   against the viewport (or, in the editor, against the preview pane), so this
   cannot key off a width — it always has to clear. */
.md2pdf-doc:not(:first-child) { padding-top: 4.75rem; }

/* ---- title block ------------------------------------------------------ */

.md2pdf-titleblock { margin: 0 0 3rem; }
.md2pdf-titleblock h1 {
  margin: 0 0 .35rem;
  font-size: 2.35em;
  line-height: 1.15;
  letter-spacing: -.015em;
}
.md2pdf-subtitle { margin: 0 0 .6rem; font-size: 1.2em; color: var(--md-muted); }
.md2pdf-byline { margin: 0; color: var(--md-muted); font-size: .95em; }
.md2pdf-byline span + span::before { content: " · "; }

/* ---- headings --------------------------------------------------------- */

.md2pdf h1, .md2pdf h2, .md2pdf h3,
.md2pdf h4, .md2pdf h5, .md2pdf h6 {
  color: var(--md-heading);
  font-weight: 700;
  line-height: 1.25;
  scroll-margin-top: 1.5rem;
}
.md2pdf-body h1   { font-size: 1.52em; margin: 1.9em 0 .65em; }
.md2pdf-body h2   { font-size: 1.30em; margin: 1.7em 0 .6em; }
.md2pdf-body h3   { font-size: 1.16em; margin: 1.5em 0 .6em; }
.md2pdf-body h4   { font-size: 1.07em; margin: 1.35em 0 .6em; letter-spacing: .02em; }
.md2pdf-body h5   { font-size: 1.00em; margin: 1.3em 0 .6em; letter-spacing: .05em; }
.md2pdf-body h6   { font-size: .94em;  margin: 1.25em 0 .6em; letter-spacing: .08em; }
.md2pdf-body > :first-child { margin-top: 0; }

/* An anchor that only shows on hover, so headings stay clean when reading.
   Absolute, not floated: a float would shift every heading by its own width. */
.md2pdf :is(h1, h2, h3, h4, h5, h6):has(> .md2pdf-anchor) { position: relative; }
.md2pdf-anchor {
  position: absolute;
  right: 100%;
  padding-right: .3em;
  font-weight: 400;
  color: var(--md-rule);
  text-decoration: none;
  opacity: 0;
  transition: opacity .12s ease;
}
:is(h1, h2, h3, h4, h5, h6):hover > .md2pdf-anchor { opacity: 1; }
.md2pdf-anchor:hover, .md2pdf-anchor:focus-visible { color: var(--md-accent); opacity: 1; }
@media (max-width: 900px) { .md2pdf-anchor { display: none; } }

/* ---- text ------------------------------------------------------------- */

.md2pdf p { margin: 0 0 1.05em; }
.md2pdf a { color: var(--md-accent); text-decoration-thickness: .06em; text-underline-offset: .18em; }
.md2pdf a:hover { text-decoration-thickness: .12em; }
.md2pdf mark { background: var(--md-mark-bg); color: var(--md-mark-fg); padding: .05em .15em; border-radius: 3px; }
.md2pdf del { color: var(--md-muted); }
.md2pdf sup, .md2pdf sub { font-size: .75em; line-height: 0; }
.md2pdf hr { border: 0; border-top: 1px solid var(--md-rule); margin: 2.4em 0; }
.md2pdf-pagebreak { border-top-style: dashed; }
.md2pdf-spacer { height: .8em; }
.md2pdf-emoji { height: 1.15em; width: 1.15em; vertical-align: -.18em; margin: 0 .04em; }
.md2pdf :is(.md2pdf-left, .md2pdf-center, .md2pdf-right) > :last-child { margin-bottom: 0; }
.md2pdf-left { text-align: left; }
.md2pdf-center { text-align: center; }
.md2pdf-right { text-align: right; }
.md2pdf-center :is(figure, .md2pdf-mermaid) { margin-inline: auto; }
.md2pdf-right :is(figure, .md2pdf-mermaid) { margin-inline: auto 0; }

/* ---- lists ------------------------------------------------------------ */

.md2pdf ul, .md2pdf ol { margin: 0 0 1.05em; padding-left: 1.6em; }
.md2pdf li { margin: .25em 0; }
.md2pdf li > ul, .md2pdf li > ol { margin-bottom: .2em; }
.md2pdf ul { list-style-type: disc; }
.md2pdf ul ul { list-style-type: square; }
.md2pdf ul ul ul { list-style-type: circle; }
.md2pdf ol { list-style-type: decimal; }
.md2pdf ol ol { list-style-type: lower-alpha; }
.md2pdf ol ol ol { list-style-type: lower-roman; }
.md2pdf li::marker { color: var(--md-muted); }
.md2pdf-tasks { list-style: none; padding-left: .2em; }
.md2pdf-tasks .md2pdf-tasks { padding-left: 1.5em; }
.md2pdf-task { display: flex; gap: .55em; align-items: baseline; }
.md2pdf-task > input {
  appearance: none;
  flex: 0 0 auto;
  width: .95em; height: .95em;
  translate: 0 .12em;
  border: 1.5px solid var(--md-muted);
  border-radius: 3px;
  background: transparent;
}
.md2pdf-task > input:checked {
  background: var(--md-ok);
  border-color: var(--md-ok);
  /* The tick is an inline SVG, so the box needs no icon font and no request. */
  background-image: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="none" stroke="white" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" d="M3.5 8.6l3 3 6-6.6"/></svg>');
  background-size: 100% 100%;
  background-repeat: no-repeat;
}
.md2pdf-task > div { flex: 1 1 auto; min-width: 0; }
.md2pdf-task > div > :last-child { margin-bottom: 0; }

/* ---- quotes & callouts ------------------------------------------------ */

.md2pdf blockquote {
  margin: 1.4em 0;
  padding: .85em 1.1em;
  border-left: 2px solid var(--md-accent);
  border-radius: 0 6px 6px 0;
  background: var(--md-quote-bg);
  color: var(--md-fg);
}
.md2pdf blockquote > :last-child { margin-bottom: 0; }

.md2pdf-adm {
  --md-adm: var(--md-adm-info);
  --md-adm-bg: var(--md-adm-info-bg);
  margin: 1.4em 0;
  padding: .8em 1em;
  border-left: 3px solid var(--md-adm);
  border-radius: 6px;
  background: var(--md-adm-bg);
}
.md2pdf-adm > :last-child { margin-bottom: 0; }
.md2pdf-adm-label {
  display: block;
  margin-bottom: .45em;
  font-size: .9em;
  font-weight: 700;
  color: var(--md-adm);
}
.md2pdf-adm-success   { --md-adm: var(--md-adm-success);   --md-adm-bg: var(--md-adm-success-bg); }
.md2pdf-adm-warning   { --md-adm: var(--md-adm-warning);   --md-adm-bg: var(--md-adm-warning-bg); }
.md2pdf-adm-tip       { --md-adm: var(--md-adm-tip);       --md-adm-bg: var(--md-adm-tip-bg); }
.md2pdf-adm-info      { --md-adm: var(--md-adm-info);      --md-adm-bg: var(--md-adm-info-bg); }
.md2pdf-adm-danger    { --md-adm: var(--md-adm-danger);    --md-adm-bg: var(--md-adm-danger-bg); }
.md2pdf-adm-note      { --md-adm: var(--md-adm-note);      --md-adm-bg: var(--md-adm-note-bg); }
.md2pdf-adm-caution   { --md-adm: var(--md-adm-caution);   --md-adm-bg: var(--md-adm-caution-bg); }
.md2pdf-adm-important { --md-adm: var(--md-adm-important); --md-adm-bg: var(--md-adm-important-bg); }

.md2pdf details {
  margin: 1.4em 0;
  padding: .75em 1em;
  border: 1px solid var(--md-rule);
  border-radius: 6px;
  background: var(--md-surface);
}
.md2pdf summary {
  cursor: pointer;
  font-weight: 700;
  list-style: none;
  display: flex;
  gap: .5em;
  align-items: center;
}
.md2pdf summary::-webkit-details-marker { display: none; }
.md2pdf summary::before {
  content: "";
  flex: 0 0 auto;
  width: 0; height: 0;
  border: .32em solid transparent;
  border-left-color: currentColor;
  transition: rotate .15s ease;
  transform-origin: 30% 50%;
}
.md2pdf details[open] > summary::before { rotate: 90deg; }
.md2pdf details > :last-child { margin-bottom: 0; }
.md2pdf details > summary + * { margin-top: .8em; }

/* ---- code ------------------------------------------------------------- */

.md2pdf code, .md2pdf kbd, .md2pdf pre {
  font-family: ui-monospace, "JetBrains Mono", "Fira Code", SFMono-Regular,
    Menlo, Consolas, "DejaVu Sans Mono", monospace;
  font-size: .875em;
}
.md2pdf :not(pre) > code {
  padding: .12em .38em;
  border-radius: 4px;
  background: var(--md-surface-2);
  overflow-wrap: anywhere;
}
.md2pdf-code {
  position: relative;
  margin: 1.4em 0;
  border: 1px solid var(--md-rule);
  border-radius: 8px;
  background: var(--md-surface);
  overflow: hidden;
}
.md2pdf-code pre {
  margin: 0;
  padding: .9em 1em .9em 0;
  overflow-x: auto;
  line-height: 1.55;
  tab-size: 2;
}
.md2pdf-code code { display: block; }
/* Line numbers come from a counter so the gutter never enters a copy. */
.md2pdf-code .md2pdf-line { counter-increment: md2pdf-line; display: block; }
.md2pdf-code code { counter-reset: md2pdf-line; }
.md2pdf-code .md2pdf-line::before {
  content: counter(md2pdf-line);
  position: sticky;
  left: 0;
  display: inline-block;
  width: 3.2em;
  margin-right: 1em;
  padding-right: .8em;
  text-align: right;
  color: var(--md-muted);
  background: var(--md-surface);
  user-select: none;
  -webkit-user-select: none;
}
.md2pdf-code[data-lang]::after {
  content: attr(data-lang);
  position: absolute;
  top: .5em; right: .6em;
  font-size: .72em;
  letter-spacing: .06em;
  text-transform: uppercase;
  color: var(--md-muted);
  pointer-events: none;
}
.md2pdf-copy {
  position: absolute;
  top: .4em; right: .4em;
  z-index: 1;
  padding: .3em .55em;
  border: 1px solid var(--md-rule);
  border-radius: 5px;
  background: var(--md-bg);
  color: var(--md-muted);
  font: inherit;
  font-size: .72em;
  cursor: pointer;
  opacity: 0;
  transition: opacity .12s ease;
}
.md2pdf-code:hover .md2pdf-copy,
.md2pdf-copy:focus-visible { opacity: 1; }
.md2pdf-copy:hover { color: var(--md-fg); }
.md2pdf-code:has(.md2pdf-copy)[data-lang]::after { right: 4.4em; }

.md2pdf-t-c { color: var(--md-t-c); font-style: italic; }
.md2pdf-t-s { color: var(--md-t-s); }
.md2pdf-t-n { color: var(--md-t-n); }
.md2pdf-t-k { color: var(--md-t-k); }
.md2pdf-t-t { color: var(--md-t-t); }
.md2pdf-t-m { color: var(--md-t-m); }

/* ---- tables ----------------------------------------------------------- */

.md2pdf-table {
  margin: 1.5em 0;
  overflow-x: auto;
  border: 1px solid var(--md-rule);
  border-radius: 8px;
}
.md2pdf table {
  width: 100%;
  border-collapse: collapse;
  font-size: .95em;
}
.md2pdf thead { background: var(--md-surface-2); }
.md2pdf th, .md2pdf td {
  padding: .6em .8em;
  text-align: left;
  vertical-align: top;
  border-bottom: 1px solid var(--md-rule);
}
.md2pdf th { font-weight: 700; }
.md2pdf tbody tr:last-child :is(td, th) { border-bottom: 0; }
.md2pdf td[align="right"], .md2pdf th[align="right"] { text-align: right; }
.md2pdf td[align="center"], .md2pdf th[align="center"] { text-align: center; }

/* ---- figures, math, diagrams ------------------------------------------ */

.md2pdf figure { margin: 1.6em auto; max-width: 100%; text-align: center; }
.md2pdf img { max-width: 100%; height: auto; }
.md2pdf figcaption { margin-top: .5em; font-size: .85em; color: var(--md-muted); }
.md2pdf-missing {
  display: inline-block;
  padding: .3em .6em;
  border: 1px dashed var(--md-rule);
  border-radius: 5px;
  color: var(--md-muted);
  font-size: .85em;
}
.md2pdf-mermaid { margin: 1.6em auto; max-width: 100%; text-align: center; }
.md2pdf-mermaid svg { max-width: 100%; height: auto; }
.md2pdf-math-block { display: block; margin: 1.4em 0; overflow-x: auto; }
.md2pdf math { font-size: 1.05em; }
.md2pdf-math-error { color: var(--md-adm-danger); }

/* ---- row layout ------------------------------------------------------- */

.md2pdf-row {
  display: grid;
  grid-template-columns: repeat(var(--md-cols, 2), minmax(0, 1fr));
  gap: 1em;
  margin: 1.4em 0;
}
.md2pdf-row > * > :last-child { margin-bottom: 0; }
@media (max-width: 640px) { .md2pdf-row { grid-template-columns: 1fr; } }

/* ---- footnotes & bibliography ----------------------------------------- */

.md2pdf-fnref { font-size: .78em; line-height: 0; vertical-align: super; }
.md2pdf-fnref a { text-decoration: none; }
.md2pdf-notes {
  margin-top: 3.5em;
  padding-top: 1.2em;
  border-top: 1px solid var(--md-rule);
  font-size: .9em;
  color: var(--md-muted);
}
.md2pdf-notes h2 { font-size: 1em; margin: 0 0 .8em; }
.md2pdf-notes ol { padding-left: 1.4em; }
.md2pdf-notes li { margin: .4em 0; }
.md2pdf-notes li > :last-child { margin-bottom: 0; }
.md2pdf-notes p { margin: 0; }
.md2pdf-backref { text-decoration: none; margin-left: .35em; }
.md2pdf-cite { text-decoration: none; font-variant-numeric: tabular-nums; }

/* ---- table of contents ------------------------------------------------ */

.md2pdf-toc-state { position: absolute; opacity: 0; pointer-events: none; }
.md2pdf-toc-btn {
  position: fixed;
  top: 1rem;
  /* Sit in the left gutter whenever the text column leaves room for it, so it
     floats beside the prose rather than on top of it. */
  left: max(1rem, calc(50% - var(--md-measure) / 2 - 8rem));
  z-index: 3;
  display: flex;
  gap: .45em;
  align-items: center;
  padding: .5em .8em;
  border: 1px solid var(--md-rule);
  border-radius: 8px;
  background: var(--md-bg);
  color: var(--md-muted);
  box-shadow: var(--md-shadow);
  font: inherit;
  font-size: .85rem;
  cursor: pointer;
  user-select: none;
  -webkit-user-select: none;
}
.md2pdf-toc-btn:hover { color: var(--md-fg); }
/* Narrow viewports get the icon alone — less of the line is covered. */
@media (max-width: 480px) {
  .md2pdf-toc-btn { font-size: 0; gap: 0; padding: .55em; }
  .md2pdf-toc-btn::before { font-size: 1rem; }
}
.md2pdf-toc-btn::before {
  content: "";
  width: 1em; height: 1em;
  background: currentColor;
  /* Three bars, drawn with gradients so no icon font is needed. */
  mask: linear-gradient(currentColor 0 0) 0 15%/100% 12% no-repeat,
        linear-gradient(currentColor 0 0) 0 50%/70% 12% no-repeat,
        linear-gradient(currentColor 0 0) 0 85%/85% 12% no-repeat;
}
.md2pdf-toc {
  position: fixed;
  top: 0; left: 0; bottom: 0;
  z-index: 4;
  width: min(20rem, 82vw);
  padding: 4.2rem 1.25rem 2rem;
  overflow-y: auto;
  overscroll-behavior: contain;
  background: var(--md-bg);
  border-right: 1px solid var(--md-rule);
  translate: -102% 0;
  transition: translate .22s cubic-bezier(.4, 0, .2, 1);
}
.md2pdf-toc-state:checked ~ .md2pdf-toc { translate: 0 0; }
.md2pdf-toc-state:checked ~ .md2pdf-toc-btn { color: var(--md-fg); }
.md2pdf-toc-state:focus-visible ~ .md2pdf-toc-btn { outline: 2px solid var(--md-accent); outline-offset: 2px; }
.md2pdf-toc-scrim {
  position: fixed;
  inset: 0;
  z-index: 3;
  background: var(--md-scrim);
  opacity: 0;
  pointer-events: none;
  transition: opacity .22s ease;
}
.md2pdf-toc-state:checked ~ .md2pdf-toc-scrim { opacity: 1; pointer-events: auto; }
.md2pdf-toc-title {
  margin: 0 0 .8em;
  font-size: .75rem;
  font-weight: 700;
  letter-spacing: .1em;
  text-transform: uppercase;
  color: var(--md-muted);
}
.md2pdf-toc ol { list-style: none; margin: 0; padding: 0; }
.md2pdf-toc a {
  display: block;
  padding: .3em .5em;
  border-radius: 5px;
  color: var(--md-fg);
  text-decoration: none;
  font-size: .9rem;
  line-height: 1.4;
}
.md2pdf-toc a:hover { background: var(--md-surface-2); color: var(--md-accent); }
.md2pdf-toc [data-level="2"] a { padding-left: 1.3em; font-size: .86rem; color: var(--md-muted); }
.md2pdf-toc [data-level="3"] a { padding-left: 2.2em; font-size: .84rem; color: var(--md-muted); }
.md2pdf-toc [data-level="4"] a,
.md2pdf-toc [data-level="5"] a,
.md2pdf-toc [data-level="6"] a { padding-left: 3.1em; font-size: .82rem; color: var(--md-muted); }

.md2pdf-toc-inline {
  margin: 1.6em 0;
  padding: 1em 1.2em;
  border: 1px solid var(--md-rule);
  border-radius: 8px;
  background: var(--md-surface);
}
.md2pdf-toc-inline ol { list-style: none; margin: 0; padding: 0; }
.md2pdf-toc-inline a { text-decoration: none; }
.md2pdf-toc-inline a:hover { text-decoration: underline; }
.md2pdf-toc-inline [data-level="2"] { padding-left: 1.2em; }
.md2pdf-toc-inline [data-level="3"] { padding-left: 2.4em; }
.md2pdf-toc-inline [data-level="4"],
.md2pdf-toc-inline [data-level="5"],
.md2pdf-toc-inline [data-level="6"] { padding-left: 3.6em; }

@media (prefers-reduced-motion: reduce) {
  .md2pdf *, .md2pdf *::before { transition-duration: 0s !important; }
}

@media print {
  .md2pdf-toc, .md2pdf-toc-btn, .md2pdf-toc-scrim, .md2pdf-copy { display: none !important; }
  .md2pdf { background: #fff; color: #000; }
  .md2pdf-doc { max-width: none; padding: 0; }
  .md2pdf-code, .md2pdf-adm, .md2pdf table { break-inside: avoid; }
}
"#;

/// Copy-to-clipboard for code blocks — the only script in the export.
/// Scoped to the fragment root so it also works inside the editor's shadow DOM.
pub(crate) const SCRIPT: &str = r#"
(function () {
  var root = document.currentScript && document.currentScript.getRootNode
    ? document.currentScript.getRootNode()
    : document;
  root.addEventListener('click', function (e) {
    var btn = e.target && e.target.closest && e.target.closest('.md2pdf-copy');
    if (!btn) return;
    var code = btn.parentNode.querySelector('code');
    if (!code || !navigator.clipboard) return;
    navigator.clipboard.writeText(code.innerText).then(function () {
      var was = btn.textContent;
      btn.textContent = btn.dataset.done;
      setTimeout(function () { btn.textContent = was; }, 1200);
    });
  });
})();
"#;
