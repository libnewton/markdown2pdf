export interface Template {
	id: string;
	name: string;
	icon: string;
	content: string;
}

const date = new Date().toISOString().split('T')[0];

// The feature demo, kept byte-identical to `tests/extended.md` (the CLI
// fixture) apart from the injected date — one document is the reference for
// both front-ends.
const WELCOME = `---
lang: en
title: md2pdf Feature Demo
authors:
  - md2pdf Team
date: ${date}
pageNumbers: "1/1"
cover: arcs
cover-color: ocean
cover-subtitle: Every feature, every option
header-left: "{title}"
header-right: "{date}"
header-height: 10mm
footer-left: md2pdf
footer-height: 10mm
bibliography: inline
bibliography-style: ieee
---

# Welcome — every feature, one document

md2pdf turns Markdown into a typeset PDF, 100% in your browser. This document is both a showcase and a reference: the first half demonstrates every syntax the renderer supports, the second half lists every option you can set.

==This sentence is highlighted== to draw the eye. **Bold**, _italic_, **_both at once_**, ~~strikethrough~~, \`inline code\`, __underline__, super^script^ and sub~script~ all work inline. A backslash escapes any character: \\*not italic\\*. Footnotes too[^demo].

[^demo]: Footnotes render as numbered notes at the foot of the page.

Line breaks are kept: every newline in the source is a line break in the PDF. Write each paragraph as one long line — as this document does — and it is justified and wrapped for you; hard-wrap the source and those wraps show up in the output. A blank line starts a new paragraph; three or more blank lines leave extra vertical space.

---

## Table of Contents

[toc]

---

## Headings (levels 1–6)

# H1 — Page Title
## H2 — Section
### H3 — Subsection
#### H4
##### H5
###### H6

The first \`#\` heading of a document becomes its title if the frontmatter has no \`title:\` — and is then dropped from the body, so it is never printed twice.

---

## Lists

### Unordered (markers cycle: • → ▪ → ◦)

- First level
- Another item
  - Second level (filled square)
  - And another
    - Third level (hollow circle)
    - More nesting
- Back to first

### Ordered (numbering cycles: 1. → a) → i))

1. First item
2. Second item
   1. Sub-item a
   2. Sub-item b
      1. Deep roman one
      2. Deep roman two
3. Third item

### Task list

- [x] Set up the project
- [x] Write the demo
- [ ] Ship to production
- [ ] Celebrate

---

## Tables

Column alignment comes from the \`:\` markers in the separator row. A column grows by appending \`+\` to its separator cell: \`---\` is one share of the width, \`---+\` two, \`---++\` three.

| Feature       | Supported | Notes                                     |
| ------------- | --------- | --------------------------------------++  |
| GFM tables    | yes       | left/right/center alignment               |
| Headers       | yes       | light grey background, rounded corners    |
| Inline markup | yes       | **bold**, \`code\`, [links](https://typst.app) |

| Left   |  Center  |  Right |
| :----- | :------: | -----: |
| left   |  middle  |  right |
| col    |   col    |    col |

---

## Code blocks (with line numbers)

Fence with three backticks or with \`~~~\`, and name the language after the opening fence to label it. Line numbers appear in the gutter automatically.

\`\`\`typescript
// TypeScript — line numbers appear in the gutter
import { markdownToTypst } from '$lib/pipeline/markdownToTypst';

export function build(md: string) {
  return markdownToTypst(md, { style: 'modern-tech' });
}
\`\`\`

\`\`\`python
# Python
def quicksort(arr):
    if len(arr) <= 1:
        return arr
    pivot = arr[len(arr) // 2]
    return (
        quicksort([x for x in arr if x < pivot])
        + [x for x in arr if x == pivot]
        + quicksort([x for x in arr if x > pivot])
    )
\`\`\`

\`\`\`
A fence without a language is set as plain preformatted text.
\`\`\`

---

## Math

Inline math like $E = mc^2$ flows with the paragraph, written between single dollars. The GitHub spelling — a dollar, a backticked expression, a dollar — renders identically: $\`\\alpha + \\beta\`$. Complex inline: $\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}$.

Block math gets its own line:

$$
\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}
$$

$$
\\sum_{n=1}^{\\infty} \\frac{1}{n^2} = \\frac{\\pi^2}{6}
$$

\`\\boxed\` frames a result, and amsmath environments line equations up on their
relation:

$$
\\boxed{X\\to Y\\to Z \\;\\Rightarrow\\; I(X;Y)\\ \\ge\\ I(X;Z)}
$$

$$
\\begin{aligned}
\\nabla \\cdot \\mathbf{E} &= \\frac{\\rho}{\\varepsilon_0} \\\\
\\nabla \\times \\mathbf{B} &= \\mu_0\\mathbf{J}
\\end{aligned}
$$

---

## Quotes & callouts

> A standard Markdown blockquote. It can contain **inline formatting** and even \`inline code\`. The left rule and tinted background come from the theme — no extra syntax required.
>
> > Quotes nest, too.

### Themed admonitions

Eight kinds — \`success\`, \`warning\`, \`tip\`, \`info\`, \`danger\`, \`note\`, \`caution\`, \`important\`. Default labels follow \`lang\` (\`de\` uses “Erfolg”, “Warnung”, “Tipp”, “Info”, “Gefahr”, “Hinweis”, “Vorsicht”, and “Wichtig”); text after the kind always replaces the default.

:::success
**Looks good.** Use \`:::success\` for confirmations, completed steps, or positive results.
:::

:::warning
**Heads-up.** Use \`:::warning\` for caveats and things that might bite.
:::

:::tip
**Pro tip.** Use \`:::tip\` for advice or shortcuts. Inline math and \`code\` both work inside admonitions.
:::

:::info
**For your information.** Use \`:::info\` for neutral context.
:::

:::danger
**Do not do this.** Use \`:::danger\` for destructive or dangerous actions.
:::

:::note Custom label
This one was opened with \`:::note Custom label\` — the trailing text replaces the "NOTE" label.
:::

:::caution
Use \`:::caution\` for a serious pitfall.
:::

:::important
Use \`:::important\` for information the reader must not miss.
:::

### Spoiler

+++++ Click to reveal
The summary can sit on the opening line, as here, or on the first line of the body. You can write **any markdown** inside, including \`code\` and lists:

- one
- two
- three
+++++

---

## Layout & alignment

Wrap content in \`:::left\`, \`:::center\`, or \`:::right\` to align prose, headings, images, display math, and diagrams. Full-width structures such as tables and code blocks keep their own layout. Wrap several blocks (separated by blank lines) in \`::::row\` to lay them out side by side as equal-width columns.

:::center
#### A centered subheading

This paragraph is centered too.
:::

::::row
First column — left-aligned paragraph.

Second column with **bold**.

Third column ends here.
::::

Use a deeper fence (four colons) when nesting other directives — e.g. two admonitions next to each other:

::::row
:::tip
Tip on the left.
:::

:::warning
Warning on the right.
:::
::::

---

## Images

Images are centered by default and fill the text width unless you give them a size. Wrap a sized image in an alignment directive to move it; its caption stays centered beneath the image. Alt text becomes the caption; leave it empty for a bare image. \`http(s)\` URLs are fetched and embedded — in the browser they go through \`fetch\` (set a CORS proxy in the settings menu if a host refuses), in the CLI through a prefetch pass before the compile.

The path may be a file next to the document or an \`http(s)\` URL — both take the same size syntax.

| Syntax                       | Result                            |
| ---------------------------++ | --------------------------------+ |
| \`![Alt](img.png)\`            | full text width, alt as caption   |
| \`![](img.png)\`               | full text width, no caption       |
| \`![Alt](img.png "=320x200")\` | 320 pt wide, 200 pt tall          |
| \`![Alt](img.png "=320x")\`    | 320 pt wide, height follows       |
| \`![Alt](<img.png =320x200>)\` | same sizing, angle-bracket form   |

:::right
![Octocat — fetched live from GitHub, sized 200×200 and right-aligned](https://octodex.github.com/images/minion.png "=200x200")
:::

---

## Emojis

Unicode emoji are rendered as Twemoji SVGs (offline, bundled): 😀 🚀 🔥 ✨ 🎉 📄 🛡️.

Shortcodes work too: :smile: :heart: :rocket: :tada: :sparkles: :warning: :white_check_mark:.

Sequences are handled as one glyph — flags 🇩🇪 🇯🇵 and families 👨‍👩‍👧 included.

---

## Links & references

Inline: [md2pdf README](https://github.com/libnewton/markdown2pdf).

Reference style: [SvelteKit][sk] and [Typst][typst] power the rendering.

Bare and pointy-bracket URLs link themselves: https://typst.app and <https://kit.svelte.dev>.

[sk]: https://kit.svelte.dev
[typst]: https://typst.app

---

## Diagrams (Mermaid)

A fence tagged \`mermaid\` is rendered as a diagram.

\`\`\`mermaid
graph LR
    Markdown-->Typst
    Typst-->SVG
    Typst-->PDF
\`\`\`

---

## Structural tokens

| Token                            | Effect                                     |
| -------------------------------+ | -----------------------------------------++ |
| \`[toc]\`                          | Outline of every heading, localised by \`lang\` |
| \`[[pagebreak]]\`                  | Start a new page here                      |
| \`---\`                            | Horizontal rule                            |
| three or more blank lines        | Extra vertical space between two paragraphs |
| \`:::left\` \`:::center\` \`:::right\` | Align a block                              |
| \`::::row\`                        | Blocks side by side, equal columns         |
| \`:::kind Optional title\`         | Admonition box                             |
| \`+++++\`                          | Spoiler box with a summary line            |

---

## Frontmatter reference

Everything the document controls lives in the YAML block at the top. All keys are optional, and hyphenated keys also accept underscores (\`header_left\`, \`cover_color\`, \`letter_to\`).

| Key                                          | Value                                         | Default             |
| -------------------------------------------+ | --------------------------------------------++ | ------------------+ |
| \`title\`                                      | text                                          | leading \`#\` heading |
| \`authors\` or \`author\`                        | one name or a list                            | none                |
| \`date\`                                       | text or a YAML date                           | none                |
| \`lang\`                                       | \`en\`, \`de\`, \`de-AT\`, …                        | \`en\`                |
| \`hide-toc-button\`                            | \`true\` to drop the HTML outline drawer        | \`false\`             |
| \`pageNumbers\` or \`page-numbers\`              | \`true\`, \`false\`, \`"1"\`, \`"1/1"\`, or a template | \`true\`              |
| \`header-left\` \`header-center\` \`header-right\` | text or an image                              | empty               |
| \`header-height\`                              | positive \`pt\`, \`mm\`, \`cm\`, or \`in\` length     | automatic           |
| \`footer-left\` \`footer-center\` \`footer-right\` | text or an image                              | centre = page number |
| \`footer-height\`                              | positive \`pt\`, \`mm\`, \`cm\`, or \`in\` length     | automatic           |
| \`cover\`                                      | \`arcs\`, \`strata\`, \`wedge\`, \`grid\`, \`true\`, \`false\` | \`false\`         |
| \`cover-color\`                                | palette name or a hex value                   | \`ocean\`             |
| \`cover-subtitle\`                             | text                                          | none                |
| \`cover-logo\`                                 | an image, placed top-right                    | none                |
| \`cover-date\`                                 | text                                          | value of \`date\`     |
| \`cover-image\`                                | an image, filling the whole cover             | none                |
| \`cover-text-color\`                           | \`white\`, \`black\`, or a hex value              | \`black\`             |
| \`bibliography\`                               | \`inline\` enables trailing BibTeX               | disabled            |
| \`bibliography-style\`                         | a Typst bibliography style                     | \`ieee\`               |
| \`letter-return\`                              | one line                                      | none                |
| \`letter-to\`                                  | one line or a list (up to six)                | none                |
| \`letter-from\`                                | one line or a list                            | none                |
| \`letter-subject\`                             | text                                          | none                |
| \`letter-date\`                                | text                                          | none                |

What each group does:

- **\`title\` / \`authors\` / \`date\`** — printed as a centered title block on page one, or moved onto the cover when one is set. They also feed the header and footer placeholders.
- **\`lang\`** — drives hyphenation, smart quotes and Typst's built-in titles: under \`lang: de\` a \`[toc]\` is headed "Inhaltsverzeichnis" instead of "Contents". A region may be appended, as in \`de-AT\`.
- **\`pageNumbers\`** — see the table below.
- **\`header-*\` / \`footer-*\`** — see *Running header & footer*. Set the matching height when a tall image or multi-line slot needs more room; the body margin grows so furniture cannot overlap it.
- **\`cover-*\`** — see *Cover page*.
- **\`bibliography\`** — see *Inline bibliography*.
- **\`letter-*\`** — any one of them switches on DIN 5008 letter mode.

### Page numbering

| \`pageNumbers\`             | Footer centre reads |
| ------------------------+ | -----------------+  |
| \`true\` or \`"1"\`           | \`3\`                 |
| \`"1/1"\`                   | \`3 / 12\`            |
| \`false\`                   | nothing             |
| \`"Page {page} of {pages}"\` | \`Page 3 of 12\`      |

Setting \`footer-center\` yourself replaces the number entirely.

---

## Running header & footer

Six optional slots — \`header-left\`, \`header-center\`, \`header-right\` and the same three for \`footer-\`. They are small, grey, have no separating rule, and start on the **second** page, so a cover or title page stays clean. This document uses four of them:

\`\`\`yaml
---
pageNumbers: "1/1"      # -> the "4 / 12" in the footer centre
header-left: "{title}"
header-right: "{date}"
header-height: 8mm
footer-left: md2pdf
footer-height: 10mm
---
\`\`\`

The height fields accept a positive \`pt\`, \`mm\`, \`cm\`, or \`in\` length. They reserve a fixed content envelope plus the normal gap from the body; leave them out for the original automatic furniture layout. Content that is taller than its configured envelope stops compilation instead of overlapping the page body.

Placeholders, usable in any slot and in a \`pageNumbers\` template:

| Placeholder            | Expands to                                |
| ---------------------+ | ---------------------------------------++ |
| \`{page}\` \`{pages}\`     | current page number, total page count     |
| \`{title}\` \`{subtitle}\` | \`title\`, \`cover-subtitle\`                 |
| \`{author}\` \`{authors}\` | the author list, comma-separated          |
| \`{date}\`               | \`date\`                                    |

Anything unrecognised is left as written.

A slot can hold a graphic instead of text, using ordinary Markdown image syntax. A remote \`https://\` URL works there too, and is fetched exactly like a body image:

\`\`\`yaml
header-right: "![](logo.png =x22)"   # =WxH in points; =x22 sets height only
\`\`\`

---

## Cover page

Page one of this document is a cover. Set \`cover:\` to one of \`arcs\`, \`strata\`, \`wedge\` or \`grid\` (plain \`true\` gives \`arcs\`). The cover counts as page one, so the first content page is numbered 2 and the header and footer begin there. Title, subtitle, authors and date all move onto it.

\`\`\`yaml
---
title: md2pdf Feature Demo
authors: [md2pdf Team]
date: 2026-05-19
cover: arcs
cover-color: ocean                     # see the palette list below
cover-subtitle: Every feature, every option
cover-logo: "![](logo.svg =x16)"       # optional, placed top-right
cover-date: May 2026                   # optional, overrides \`date\` here only
---
\`\`\`

The four geometries fill the bottom of the page: \`arcs\` (quarter-discs from the bottom-left), \`strata\` (slanted bands), \`wedge\` (one layered diagonal), \`grid\` (a fading dot lattice over solid bars).

\`cover-color\` takes a named palette — \`ocean\`, \`ink\`, \`ember\`, \`plum\`, \`forest\`, \`crimson\`, \`teal\`, \`gold\`, or \`rub\` (Ruhr-Universität Bochum house colours) — or any hex value such as \`"#0074de"\`, from which the lighter tones are derived. The palette only tints the geometry and the hairline under the title. Cover mode and letter mode are mutually exclusive — letter mode wins.

### A ready-made cover

\`cover-image\` puts an existing design — an exported A4 background, a company template — behind the whole cover page, edge to edge. It switches the cover on by itself, and it replaces the geometry, so \`cover:\` is neither needed nor drawn:

\`\`\`yaml
---
title: Quarterly Report
cover-image: cover-a4.png              # or "![](https://…/cover.png)" for a remote one
cover-text-color: "#ffffff"            # dark artwork needs light text
---
\`\`\`

The path is resolved like any other image: relative to the document in the CLI, and to the uploaded asset path in the web app — drop the image into the editor, then move the inserted \`images/…\` path into \`cover-image:\`. A remote cover needs the Markdown form (\`"![](url)"\`), which is what gets it prefetched. Sizing is ignored: the image always covers the page, cropped rather than distorted if it is not A4. PDF covers are not supported — export them as PNG or SVG first.

Title, subtitle, authors and date stay where they always are, so \`cover-text-color\` is usually all a dark background needs; it takes \`white\`, \`black\`, or a hex value. The hairline keeps its \`cover-color\` tint.

---

## German letter mode (DIN 5008)

Add any of the \`letter-*\` fields and the first page switches to a DIN 5008 Form B layout: address window at 25 mm / 45 mm so it lines up with a DIN long envelope, sender info on the right, subject and place-date on the same line, body content from 98.46 mm down. Page margins switch to 20 mm on both sides. All fields are optional and independent.

\`\`\`yaml
---
title: Mietvertrag-Kündigung
letter-return: Anna Beispiel, Lindenweg 7, 10115 Berlin
letter-to:
  - Hausverwaltung Müller GmbH
  - z. Hd. Frau Schmidt
  - Friedrichstraße 100
  - 10117 Berlin
letter-from:
  - Anna Beispiel
  - Lindenweg 7
  - 10115 Berlin
  - "Tel.: 030 1234567"
letter-subject: "Kündigung des Mietvertrags zum 31.08.2026"
letter-date: "Berlin, den 17.05.2026"
---
\`\`\`

The body of the letter is just Markdown — everything in this document works there as well.

---

## Page breaks & spacing

Use \`---\` for a horizontal rule. Leaving three or more blank lines in the source opens up extra vertical space:



…which is the small gap above this line. The explicit token \`[[pagebreak]]\` starts a new page right here:

[[pagebreak]]

## You are now on a new page

Anything after the token continues on the next page. Combine with sections to keep chapters cleanly separated.

> **One more tip**: Press <kbd>Ctrl</kbd>+<kbd>Enter</kbd> in the editor to trigger a compile immediately, even when live preview is paused.

---

## Inline bibliography

Set \`bibliography: inline\` to cite a trailing BibTeX entry as \`[@md2pdf]\`. This sentence contains an actual citation [@md2pdf]. Numeric citation content is always blue while brackets and punctuation stay black, including under non-IEEE styles. The bibliography is generated automatically, headed “Referenzen” for \`lang: de\` and “References” otherwise. \`bibliography-style\` selects a Typst style and defaults to \`ieee\`. BibTeX-looking examples inside fenced code remain ordinary content.

@misc{md2pdf,
  author = {md2pdf contributors},
  title = {md2pdf},
  year = {2026},
  url = {https://github.com/typst/typst}
}
`;

export const PDF_TEMPLATES: Template[] = [
	{ id: 'welcome', name: 'Feature Demo', icon: '🚀', content: WELCOME },
];
