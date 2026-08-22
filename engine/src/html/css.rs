//! The document stylesheet, inlined into every rendered document.
//!
//! Written once and shared by the standalone export and the editor's preview
//! pane, which mounts the same markup in a shadow root — hence `:root, :host`
//! on the token block, so the variables land either way.
//!
//! Colours track `styles/modern-tech.typ` and `admonitions.typ` so a document
//! reads the same in both outputs. Every colour is a variable with a dark
//! counterpart; nothing else in the sheet hardcodes one.

use std::sync::OnceLock;

/// The stylesheet. The custom-property block is generated from `tokens.rs`, so
/// the palette has exactly one definition — the same one the Typst templates
/// read. Assembled once per plugin instance; a render never pays for it.
pub(crate) fn style() -> &'static str {
    static SHEET: OnceLock<String> = OnceLock::new();
    SHEET.get_or_init(|| format!("{PRELUDE}{}{REST}", super::tokens::declarations()))
}

const PRELUDE: &str = r#"
/* One token block. `light-dark()` picks a side from the element's
   `color-scheme`, so the OS preference and the explicit `data-theme` override
   share a single set of definitions instead of a light copy and a dark copy. */
:root, :host { color-scheme: light dark; }
:root, :host, .md2pdf {
"#;

const REST: &str = r#"}
/* An explicit override may sit on the fragment root, the document root, or
   the shadow host the editor mounts it in — all three must win. */
[data-theme="light"], :host([data-theme="light"]) { color-scheme: light; }
[data-theme="dark"], :host([data-theme="dark"]) { color-scheme: dark; }

/* ---- shell ------------------------------------------------------------ */

.md2pdf {
  --md-sans: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto,
    "IBM Plex Sans", "Helvetica Neue", Arial, sans-serif;
  --md-mono: ui-monospace, "JetBrains Mono", "Fira Code", SFMono-Regular,
    Menlo, Consolas, "DejaVu Sans Mono", monospace;
  background: var(--md-bg);
  color: var(--md-fg);
  font-family: var(--md-sans);
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
.md2pdf-doc:not(:first-child) { padding-top: 4rem; }

/* Announced, never seen — the outline button is an icon. */
.md2pdf-sr {
  position: absolute;
  width: 1px; height: 1px;
  margin: -1px;
  padding: 0;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}

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
  font-family: var(--md-mono);
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

/* Live only in the editor's preview; the download's boxes are disabled. */
.md2pdf-task > input:not(:disabled) { cursor: pointer; }
.md2pdf-task > input:focus-visible { outline: 2px solid var(--md-accent); outline-offset: 2px; }

.md2pdf-t-c { color: var(--md-t-c); font-style: italic; }
.md2pdf-t-s { color: var(--md-t-s); }
.md2pdf-t-n { color: var(--md-t-n); }
.md2pdf-t-k { color: var(--md-t-k); }
.md2pdf-t-t { color: var(--md-t-t); }
.md2pdf-t-m { color: var(--md-t-m); }
.md2pdf-t-f { color: var(--md-t-f); }
.md2pdf-t-o { color: var(--md-t-o); }
.md2pdf-t-p { color: var(--md-t-p); }
.md2pdf-t-v { color: var(--md-t-v); }

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
.md2pdf-mermaid img { display: inline-block; max-width: 100%; height: auto; }
/* No scroll container: `overflow-x` would drag `overflow-y` to `auto` with it,
   and every formula paints a few pixels outside its box, so each one ended up
   scrollable and clipped. Overflow stays visible instead. */
.md2pdf-math-block { display: block; margin: 1.4em 0; text-align: center; }
.md2pdf-math-error { color: var(--md-adm-danger); }
/* `\boxed`. MathML Core has no boxing element and math-core emits no
   equivalent, so the border is drawn here, around the whole formula.
   `display` is deliberately untouched: a <math> element lays its children out
   as maths only while it keeps its `block math` / `inline math` display, and
   overriding it to `inline-block` stacks every glyph vertically instead.
   `fit-content` tightens the box without going near that. */
.md2pdf-math-boxed math {
  width: fit-content;
  margin-inline: auto;
  /* Wider on the right: a formula's last glyph carries italic correction that
     `fit-content` does not measure, so equal padding looks lopsided. */
  padding: .45em .85em .45em .7em;
  border: 1px solid var(--md-rule);
  border-radius: 4px;
}
/* Same face the PDF typesets math with. Without a font carrying a MATH table
   the browser lays MathML out with the body font — stretched braces, wrong
   radicals, flat fractions — so the export embeds it and the app serves it. */
.md2pdf math { font-size: 1.05em; font-family: "NewCM Math", math; }
/* `\text` and friends are meant to read as prose, not as math. */
.md2pdf mtext { font-family: var(--md-sans); }
.md2pdf mtext span.math-core-sans-serif-font { font-family: var(--md-sans); }
.md2pdf mtext span.math-core-serif-font { font-family: ui-serif, Georgia, serif; }

/* Rendering fixes math-core's output requires. Cell padding is the spec's, not
   Firefox's; the accent gap matches Firefox in the other two engines. */
.md2pdf mtd { padding: 0.5ex 0.4em; }
.md2pdf mtr > mtd:first-child { padding-left: 0; }
.md2pdf mtr > mtd:last-child { padding-right: 0; }
.md2pdf mover > :nth-child(2) { margin-bottom: 0.05em; }
/* `menclose` is unimplemented in Chromium; math-core emits an empty <mrow> per
   strike so the line can be drawn here instead. */
.md2pdf menclose { position: relative; }
.md2pdf [class^="menclose-"] {
  position: absolute;
  inset: 0;
  background-repeat: no-repeat;
}
.md2pdf .menclose-horizontalstrike {
  background-image: linear-gradient(currentColor 0 0);
  background-size: 100% 1px;
  background-position: 0 50%;
}
.md2pdf .menclose-updiagonalstrike {
  background-image: linear-gradient(to top right, transparent calc(50% - .5px), currentColor 50%, transparent calc(50% + .5px));
  background-size: 100% 100%;
}
.md2pdf .menclose-downdiagonalstrike {
  background-image: linear-gradient(to bottom right, transparent calc(50% - .5px), currentColor 50%, transparent calc(50% + .5px));
  background-size: 100% 100%;
}

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
.md2pdf-cite a { text-decoration: none; }

.md2pdf-theme-toggle {
  position: fixed;
  right: 1rem;
  bottom: 1rem;
  z-index: 3;
  width: 2.35rem;
  height: 2.35rem;
  border: 1px solid var(--md-rule);
  border-radius: 50%;
  background: var(--md-bg);
  color: var(--md-muted);
  box-shadow: var(--md-shadow);
  cursor: pointer;
}
.md2pdf-theme-toggle:hover { color: var(--md-fg); }
.md2pdf-theme-toggle:focus-visible { outline: 2px solid var(--md-accent); outline-offset: 2px; }
.md2pdf-theme-moon, .md2pdf-theme-sun {
  position: relative;
  box-sizing: border-box;
  width: 1rem;
  height: 1rem;
  margin: auto;
}
.md2pdf-theme-moon {
  border: 1.6px solid currentColor;
  border-radius: 50%;
}
.md2pdf-theme-moon::after {
  content: "";
  position: absolute;
  width: .8rem;
  height: .8rem;
  left: .28rem;
  top: -.12rem;
  border-radius: 50%;
  background: var(--md-bg);
}
.md2pdf-theme-sun {
  width: .65rem;
  height: .65rem;
  border: 1.5px solid currentColor;
  border-radius: 50%;
}
.md2pdf-theme-sun::before, .md2pdf-theme-sun::after {
  content: "";
  position: absolute;
  left: 50%;
  top: 50%;
  width: 1.15rem;
  height: 1.5px;
  background: linear-gradient(to right, currentColor 0 .22rem, transparent .22rem .93rem, currentColor .93rem);
  transform: translate(-50%, -50%);
}
.md2pdf-theme-sun::after { rotate: 90deg; }
.md2pdf-theme-sun { display: none; }
[data-theme="dark"] .md2pdf-theme-moon { display: none; }
[data-theme="dark"] .md2pdf-theme-sun { display: block; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme]) .md2pdf-theme-moon { display: none; }
  :root:not([data-theme]) .md2pdf-theme-sun { display: block; }
}

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
  align-items: center;
  justify-content: center;
  width: 2.1rem;
  height: 2.1rem;
  border: 1px solid var(--md-rule);
  border-radius: 8px;
  background: var(--md-bg);
  color: var(--md-muted);
  box-shadow: var(--md-shadow);
  font: inherit;
  font-size: 1rem;
  cursor: pointer;
  user-select: none;
  -webkit-user-select: none;
}
.md2pdf-toc-btn:hover { color: var(--md-fg); }
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
  .md2pdf-toc, .md2pdf-toc-btn, .md2pdf-toc-scrim, .md2pdf-copy,
  .md2pdf-theme-toggle { display: none !important; }
  .md2pdf { background: #fff; color: #000; }
  .md2pdf-doc { max-width: none; padding: 0; }
  .md2pdf-code, .md2pdf-adm, .md2pdf table { break-inside: avoid; }
}
"#;

/// The only script in the export: copy-to-clipboard on code blocks, and
/// in-document links.
///
/// It binds one listener to `document` and resolves the scope from the element
/// that was clicked. That is what makes it work in the editor too: the same
/// markup lives in a shadow root there, where the browser cannot resolve a
/// `#fragment` because the ids are not in the document — and where
/// `document.currentScript` is null, so the script cannot find its own root.
pub(crate) const SCRIPT: &str = r#"
(function () {
  if (window.__md2pdfBound) return;
  window.__md2pdfBound = true;
  document.addEventListener('click', function (e) {
    var el = e.composedPath ? e.composedPath()[0] : e.target;
    if (!el || !el.closest) return;

    var btn = el.closest('.md2pdf-copy');
    if (btn) {
      var code = btn.parentNode.querySelector('code');
      if (!code || !navigator.clipboard) return;
      navigator.clipboard.writeText(code.innerText).then(function () {
        var was = btn.textContent;
        btn.textContent = btn.dataset.done;
        setTimeout(function () { btn.textContent = was; }, 1200);
      });
      return;
    }

    var theme = el.closest('.md2pdf-theme-toggle');
    if (theme) {
      var html = document.documentElement;
      var current = html.dataset.theme;
      if (current !== 'light' && current !== 'dark') {
        current = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
      }
      html.dataset.theme = current === 'dark' ? 'light' : 'dark';
      return;
    }

    var link = el.closest("a[href^='#']");
    if (!link) return;
    var root = link.getRootNode();
    if (!root.getElementById) return;
    // Following an outline entry should also put the drawer away.
    var toggle = root.getElementById('md2pdf-toc-state');
    if (toggle) toggle.checked = false;
    var target = root.getElementById(decodeURIComponent(link.hash.slice(1)));
    // In a real document the browser handles the jump, and the history entry
    // with it; only a shadow root needs us to scroll it ourselves.
    if (target && root !== document) {
      e.preventDefault();
      target.scrollIntoView({ block: 'start', behavior: 'smooth' });
    }
  });
})();
"#;
