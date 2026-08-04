#let _css = ```css
:root {
  color-scheme: light;
  --bg: #f4f6f8;
  --surface: #ffffff;
  --surface-raised: #ffffff;
  --text: #1a222d;
  --muted: #5c6675;
  --border: #d7dde5;
  --accent: #245fc5;
  --accent-soft: #e8f0ff;
  --code: #eef1f5;
  --quote: #f7f9fc;
  --shadow: 0 18px 48px #1a222d1f;
  --info: #1f63b5;
  --info-bg: #edf5ff;
  --tip: #16734a;
  --tip-bg: #eaf8f1;
  --warning: #8a5a00;
  --warning-bg: #fff6dc;
  --danger: #b42338;
  --danger-bg: #fff0f2;
}
:root[data-theme="dark"] {
  color-scheme: dark;
  --bg: #0d1218;
  --surface: #141b24;
  --surface-raised: #1a2330;
  --text: #edf1f6;
  --muted: #aab4c2;
  --border: #303b49;
  --accent: #82b1ff;
  --accent-soft: #172b47;
  --code: #0b1016;
  --quote: #18212c;
  --shadow: 0 20px 56px #00000066;
  --info: #8bbcff;
  --info-bg: #142a43;
  --tip: #72d6a3;
  --tip-bg: #123126;
  --warning: #f1c75b;
  --warning-bg: #352a11;
  --danger: #ff929f;
  --danger-bg: #3a1d24;
}
*, *::before, *::after { box-sizing: border-box; }
html { scroll-behavior: smooth; background: var(--bg); }
body {
  margin: 0;
  min-width: 0;
  background: var(--bg);
  color: var(--text);
  font: 400 clamp(1rem, .97rem + .14vw, 1.075rem)/1.72 system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
}
.md-document {
  width: min(100% - clamp(2rem, 8vw, 8rem), 75ch);
  margin-inline: auto;
  padding-block: clamp(3.5rem, 8vw, 7rem);
}
.md-header { margin-block-end: clamp(3rem, 7vw, 5.5rem); }
.md-header h1 {
  margin: 0;
  max-width: 24ch;
  font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-size: clamp(2.2rem, 1.75rem + 2vw, 3.45rem);
  font-weight: 720;
  line-height: 1.08;
  letter-spacing: -.027em;
}
.md-byline { margin: 1.1rem 0 0; color: var(--muted); font-size: .97rem; }
h1, h2, h3, h4, h5, h6 {
  color: var(--text);
  line-height: 1.2;
  text-wrap: balance;
  scroll-margin-top: 2rem;
}
h2 { margin: 3.6rem 0 1rem; font-size: clamp(1.65rem, 1.45rem + .8vw, 2.1rem); letter-spacing: -.025em; }
h3 { margin: 2.8rem 0 .8rem; font-size: 1.42rem; letter-spacing: -.018em; }
h4 { margin: 2.2rem 0 .65rem; font-size: 1.18rem; }
h5, h6 { margin: 1.9rem 0 .55rem; font-size: 1rem; letter-spacing: .025em; }
p { margin: 0 0 1.25em; }
a { color: var(--accent); text-underline-offset: .18em; text-decoration-thickness: .08em; }
a:hover { text-decoration-thickness: .14em; }
:focus-visible { outline: 3px solid var(--accent); outline-offset: 3px; }
strong { font-weight: 700; }
mark { border-radius: .2em; padding-inline: .12em; background: #ffe28a; color: #332600; }
:root[data-theme="dark"] mark { background: #755d14; color: #fff3bd; }
hr { margin: 2.6rem 0; border: 0; border-top: 1px solid var(--border); }
blockquote {
  margin: 2rem 0;
  padding: 1rem 1.25rem;
  border-left: .22rem solid var(--accent);
  border-radius: 0 .65rem .65rem 0;
  background: var(--quote);
  color: var(--text);
}
blockquote > :last-child { margin-bottom: 0; }
ul, ol { margin: 0 0 1.35rem; padding-left: 1.5rem; }
li { margin-block: .38rem; padding-left: .2rem; }
li::marker { color: var(--muted); }
code, pre { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
code {
  border: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
  border-radius: .35rem;
  background: var(--code);
  padding: .12em .32em;
  font-size: .88em;
  overflow-wrap: anywhere;
}
pre {
  max-width: 100%;
  margin: 1.7rem 0;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: .8rem;
  background: var(--code);
  padding: 1rem 1.15rem;
  line-height: 1.58;
  tab-size: 2;
}
pre code { border: 0; padding: 0; background: transparent; overflow-wrap: normal; }
table {
  display: block;
  width: max-content;
  max-width: 100%;
  margin: 1.8rem 0;
  overflow-x: auto;
  border-spacing: 0;
  border-collapse: separate;
  border: 1px solid var(--border);
  border-radius: .7rem;
}
th, td { min-width: 8rem; padding: .72rem .85rem; border-bottom: 1px solid var(--border); text-align: left; vertical-align: top; }
th { background: var(--accent-soft); font-size: .9rem; font-weight: 700; }
tr:last-child > * { border-bottom: 0; }
th + th, td + td { border-left: 1px solid var(--border); }
img, svg { display: block; max-width: 100%; height: auto; }
.twemoji { display: inline-flex; width: 1em; height: 1em; vertical-align: -.15em; }
.twemoji img { width: 1em; height: 1em; }
figure { margin: 2rem 0; }
figcaption { margin-top: .65rem; color: var(--muted); font-size: .88rem; text-align: center; }
math, .md-align { max-width: 100%; overflow-x: auto; }
.md-align { margin-block: 1.5rem; }
.md-align-left { text-align: left; }
.md-align-center { text-align: center; }
.md-align-right { text-align: right; }
.md-align-center > img, .md-align-center > figure { margin-inline: auto; }
.md-align-right > img, .md-align-right > figure { margin-left: auto; }
.md-boxed { white-space: nowrap; }
.md-row { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(16rem, 100%), 1fr)); gap: 1rem; margin-block: 1.75rem; }
.md-row > * { min-width: 0; }
.md-blank { height: .8rem; }
.md-pagebreak { margin-block: 4rem; border-top-style: dashed; }
.md-admonition {
  margin: 1.8rem 0;
  padding: 1rem 1.15rem;
  border: 1px solid color-mix(in srgb, currentColor 30%, transparent);
  border-left-width: .28rem;
  border-radius: .7rem;
}
.md-admonition-title { margin: 0 0 .45rem; font-size: .86rem; font-weight: 750; letter-spacing: .035em; text-transform: uppercase; }
.md-admonition > :last-child { margin-bottom: 0; }
.md-admonition-info, .md-admonition-note { color: var(--info); background: var(--info-bg); }
.md-admonition-tip, .md-admonition-success { color: var(--tip); background: var(--tip-bg); }
.md-admonition-warning { color: var(--warning); background: var(--warning-bg); }
.md-admonition-danger, .md-admonition-error { color: var(--danger); background: var(--danger-bg); }
.md-admonition-caution { color: var(--danger); background: var(--danger-bg); }
.md-admonition-important { color: var(--accent); background: var(--accent-soft); }
.md-admonition-body { color: var(--text); }
.md-spoiler { margin: 1.7rem 0; border: 1px solid var(--border); border-radius: .7rem; background: var(--surface); }
.md-spoiler summary { cursor: pointer; padding: .85rem 1rem; font-weight: 650; }
.md-spoiler[open] summary { border-bottom: 1px solid var(--border); }
.md-spoiler-body { padding: 1rem; }
.md-spoiler-body > :last-child { margin-bottom: 0; }
.md-task { width: 1rem; height: 1rem; margin: 0 .48rem 0 0; accent-color: var(--accent); vertical-align: -.12rem; }
.md-task-item { cursor: default; }
.md-toc {
  position: fixed;
  z-index: 20;
  top: 1rem;
  left: 1rem;
  max-width: min(18rem, calc(100vw - 2rem));
}
.md-toc[open] {
  width: min(18rem, calc(100vw - 2rem));
  max-height: min(32rem, calc(100vh - 2rem));
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: .65rem;
  background: color-mix(in srgb, var(--surface) 96%, transparent);
  box-shadow: var(--shadow);
  backdrop-filter: blur(14px);
}
.md-toc summary {
  display: flex;
  width: 2.55rem;
  height: 2.55rem;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  list-style: none;
  border: 1px solid var(--border);
  border-radius: .65rem;
  background: var(--surface-raised);
  color: var(--text);
  box-shadow: 0 5px 18px #0000001a;
}
.md-toc summary::-webkit-details-marker { display: none; }
.md-toc[open] summary {
  position: sticky;
  top: 0;
  z-index: 1;
  width: 100%;
  height: 2.7rem;
  justify-content: flex-start;
  gap: .55rem;
  padding-inline: .75rem;
  border: 0;
  border-bottom: 1px solid var(--border);
  border-radius: .65rem .65rem 0 0;
  background: var(--surface);
  box-shadow: none;
}
.md-toc-icon { font-size: 1.05rem; line-height: 1; }
.md-toc-label {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}
.md-toc[open] .md-toc-label {
  position: static;
  width: auto;
  height: auto;
  clip-path: none;
  color: var(--text);
  font-size: .78rem;
  font-weight: 700;
  letter-spacing: .02em;
}
.md-toc nav { padding: .4rem .7rem .75rem; font-size: .8rem; line-height: 1.35; }
.md-toc nav ol { margin: 0; padding-left: 1rem; }
.md-toc nav li { margin-block: .16rem; padding-left: 0; }
.md-toc nav a { text-decoration: none; }
.md-toc nav a:hover { text-decoration: underline; }
.md-theme-toggle {
  position: fixed;
  z-index: 21;
  top: 1rem;
  right: 1rem;
  height: 2.55rem;
  min-width: 4.5rem;
  padding-inline: .75rem;
  border: 1px solid var(--border);
  border-radius: .65rem;
  background: var(--surface-raised);
  color: var(--text);
  box-shadow: 0 5px 18px #0000001a;
  cursor: pointer;
  font: 700 .75rem/1 system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
.md-theme-toggle:hover { border-color: var(--accent); color: var(--accent); }
.footnote, [role="doc-endnotes"] { color: var(--muted); font-size: .9rem; }
@media (max-width: 640px) {
  .md-document { width: min(100% - 2rem, 75ch); padding-block: 5.25rem 3.5rem; }
  .md-toc { top: .75rem; left: .75rem; max-width: min(18rem, calc(100vw - 1.5rem)); }
  .md-toc[open] { width: min(18rem, calc(100vw - 1.5rem)); max-height: min(32rem, calc(100vh - 1.5rem)); }
  .md-theme-toggle { top: .75rem; right: .75rem; }
  th, td { min-width: 7rem; }
}
@media print {
  :root { --bg: #fff; --surface: #fff; --text: #000; --muted: #444; --border: #bbb; }
  .md-document { width: auto; padding: 0; }
  .md-theme-toggle, .md-toc { display: none; }
  .md-pagebreak { break-before: page; border: 0; margin: 0; }
  a { color: inherit; }
}
@media (prefers-reduced-motion: reduce) { html { scroll-behavior: auto; } }
```.text

#let _script = ```js
(() => {
  const root = document.documentElement;
  const button = document.getElementById('md-theme-toggle');
  const key = 'md2pdf-theme';
  let theme = 'light';
  try { theme = localStorage.getItem(key) === 'dark' ? 'dark' : 'light'; } catch (_) {}
  const apply = (next, persist = false) => {
    theme = next === 'dark' ? 'dark' : 'light';
    root.dataset.theme = theme;
    button.textContent = theme === 'dark' ? 'Light' : 'Dark';
    button.setAttribute('aria-label', theme === 'dark' ? 'Use light theme' : 'Use dark theme');
    button.setAttribute('aria-pressed', String(theme === 'dark'));
    if (persist) { try { localStorage.setItem(key, theme); } catch (_) {} }
  };
  apply(theme);
  button.addEventListener('click', () => {
    apply(theme === 'dark' ? 'light' : 'dark', true);
    if (parent !== window) parent.postMessage({ type: 'md2pdf-theme-change', theme }, '*');
  });
  addEventListener('message', event => {
    if (event.data?.type === 'md2pdf-theme') apply(event.data.theme);
    if (event.data?.type === 'md2pdf-scroll-restore') scrollTo(0, event.data.top || 0);
  });
  let queued = false;
  addEventListener('scroll', () => {
    if (parent === window || queued) return;
    queued = true;
    requestAnimationFrame(() => {
      parent.postMessage({ type: 'md2pdf-scroll', top: scrollY }, '*');
      queued = false;
    });
  }, { passive: true });
  document.addEventListener('click', event => {
    const link = event.target.closest('.md-toc a');
    if (link && innerWidth < 641) link.closest('details')?.removeAttribute('open');
  });
  if (parent !== window) parent.postMessage({ type: 'md2pdf-ready' }, '*');
})();
```.text

#let article(title: "", authors: (), ..args, doc) = {
  let lang = args.at("lang", default: "en")
  let region = args.at("region", default: none)
  let language = if region == none { lang } else { lang + "-" + region }
  let date = args.at("date", default: none)
  let footnotes = state("md2pdf-html-footnotes", ())
  let content = {
    show heading: item => html.elem("h" + str(item.level), item.body)
    show quote: item => html.blockquote(item.body)
    show footnote: note => {
      let index = footnotes.get().len() + 1
      let source = label("md2pdf-footnote-source-" + str(index))
      let target = label("md2pdf-footnote-target-" + str(index))
      footnotes.update(items => items + ((body: note.body, source: source, target: target),))
      [#super(link(target, str(index))) #source]
    }
    doc
  }
  html.html(lang: language, {
    html.head({
      html.meta(charset: "utf-8")
      html.meta(name: "viewport", content: "width=device-width, initial-scale=1")
      html.meta(name: "color-scheme", content: "light dark")
      if authors.len() > 0 { html.meta(name: "author", content: authors.join(", ")) }
      if date != none { html.meta(name: "date", content: date) }
      html.title(if title == "" { "md2pdf document" } else { title })
      html.style(_css)
    })
    html.body({
      html.elem("button", attrs: (
        id: "md-theme-toggle",
        class: "md-theme-toggle",
        type: "button",
        title: "Use dark theme",
        "aria-label": "Use dark theme",
        "aria-pressed": "false",
      ), [Dark])
      html.main(class: "md-document", {
        if title != "" or authors.len() > 0 or date != none {
          html.header(class: "md-header", {
            if title != "" { html.h1(title) }
            if authors.len() > 0 or date != none {
              html.p(class: "md-byline", {
                if authors.len() > 0 { html.span(class: "md-authors", authors.join(", ")) }
                if authors.len() > 0 and date != none { [ · ] }
                if date != none { html.elem("time", attrs: (datetime: date), date) }
              })
            }
          })
        }
        content
        context {
          let final = footnotes.final()
          if final.len() > 0 {
            html.elem("section", attrs: (class: "md-footnotes", role: "doc-endnotes"), {
              html.hr()
              html.ol({
                for note in final {
                  html.li[
                    #note.body #note.target #link(note.source)[↩]
                  ]
                }
              })
            })
          }
        }
      })
      html.script(_script)
    })
  })
}

#let md-align(kind, boxed: false, body) = html.div(
  class: "md-align md-align-" + kind + if boxed { " md-boxed" } else { "" },
  body,
)

#let md-row(..cells) = html.div(class: "md-row", {
  for cell in cells.pos() { html.div(cell) }
})

#let md-task-list(task-item, ..items) = html.ul(class: "md-task-list", {
  for item in items.pos() {
    html.li(task-item(item.checked, item.body))
  }
})

#let md-rule() = html.hr()
#let md-toc(lang: "en") = html.details(class: "md-toc", {
  html.summary({
    html.span(class: "md-toc-icon", [☰])
    html.span(class: "md-toc-label", if lang == "de" { [Inhalt] } else { [Contents] })
  })
  outline(title: none, indent: auto)
})
#let md-pagebreak() = html.hr(class: "md-pagebreak")
#let md-blank() = html.div(class: "md-blank")

#let md-image(
  asset,
  path,
  width: auto,
  height: auto,
  alt: none,
  caption: none,
  align: "center",
) = md-align(align, {
  let graphic = asset(path, width: width, height: height, alt: alt)
  if caption == none {
    graphic
  } else {
    html.figure({
      graphic
      html.figcaption(caption)
    })
  }
})

#let admonition(kind: "info", title: "", lang: "en", body) = {
  let names = if lang == "de" {
    (info: "Hinweis", note: "Hinweis", tip: "Tipp", success: "Erfolg", warning: "Warnung", caution: "Vorsicht", important: "Wichtig", danger: "Gefahr", error: "Fehler")
  } else {
    (info: "Note", note: "Note", tip: "Tip", success: "Success", warning: "Warning", caution: "Caution", important: "Important", danger: "Danger", error: "Error")
  }
  let label = if title != "" { title } else { names.at(kind, default: kind) }
  html.aside(class: "md-admonition md-admonition-" + kind, {
    html.p(class: "md-admonition-title", label)
    html.div(class: "md-admonition-body", body)
  })
}

#let spoiler(summary: "spoiler", body) = html.details(class: "md-spoiler", {
  html.summary(summary)
  html.div(class: "md-spoiler-body", body)
})

#let task-item(checked, body) = {
  html.label(class: "md-task-item", {
    html.input(class: "md-task", type: "checkbox", checked: checked, disabled: true)
    body
  })
}
