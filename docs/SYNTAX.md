# md2pdf syntax reference

md2pdf uses CommonMark and GFM, but preserves every source newline as a visible
line break. A blank line still starts a new paragraph. Raw HTML is escaped
except for `<br>`, which is accepted in prose and table cells.

## Inline syntax

- `**bold**`, `_italic_`, `~~strike~~`, `` `code` ``, links, images and autolinks
- `==highlight==`, `__underline__`, `^superscript^`, `~subscript~`
- Footnotes: `text[^note]` and `[^note]: definition`
- Math: `$x^2$` inline and `$$...$$` as a block
- Emoji characters and `:shortcodes:` use Twemoji when available
- Citations: `[@key]` when `bibliography: inline` is enabled

## Headings and navigation

All body headings from `#` through `######` receive Unicode-aware IDs. Link to
`## My Section` with `[jump](#my-section)`. Add a stable invisible ID with
`## My Section {#overview}` and link with `[jump](#overview)`. IDs start with a
letter or number and then use letters, numbers, `-`, `_`, `.` or `:`. Invalid
`{#...}` text stays visible. A leading H1 used as the document title is not a
body-section target.

`[toc]` inserts an outline. `[[pagebreak]]` starts a new PDF page and becomes a
dashed separator in HTML.

## Tables

GFM tables support alignment. Append `+` characters to separator cells to set
relative column widths:

```markdown
| Narrow | Wide |
| ------ | ---+++ |
| one | three shares |
```

Rows stay together across PDF page boundaries while the table itself can span
pages. Use `<br>` for a line break inside a cell.

## Blocks

Admonitions use `:::kind` and a closing `:::`. Kinds are `info`, `note`,
`tip`, `warning`, `danger`, `success`, `question`, `example`, `quote`,
`abstract`, `todo`, `caution` and `important`. Text after the kind is a custom
title.

```markdown
:::warning Optional title
Body with **Markdown**.
:::
```

`:::left`, `:::center` and `:::right` align their contents. `::::row` lays its
top-level blocks out as equal columns and closes with `::::`. Spoilers use
`+++++ Summary`, a Markdown body and a closing `+++++`.

Fenced code blocks are highlighted by language. A `mermaid` fence renders a
diagram. Images accept HackMD dimensions such as `![alt](image.png "=200x120")`
or `![alt](<image.png =200x120>)`. Remote HTTP(S) images are fetched by the
host under size, timeout and public-address restrictions.

## YAML frontmatter

Put frontmatter at the beginning between `---` lines. All keys are optional.

- Document: `title`, `subtitle`, `authors`/`author`, `date`, `lang`
- HTML: `hide-toc-button`, `hide-theme-toggle`
- Pages: `pageNumbers`/`page-numbers`
- Furniture: `header-left`, `header-center`, `header-right`, `header-height`,
  `footer-left`, `footer-center`, `footer-right`, `footer-height`
- Cover: `cover`, `cover-color`, `cover-subtitle`, `cover-logo`, `cover-date`,
  `cover-image`, `cover-text-color`
- Citations: `bibliography: inline`, `bibliography-style`
- DIN 5008 letter: `letter-return`, `letter-to`, `letter-from`,
  `letter-subject`, `letter-date`

Hyphenated PDF keys also accept underscores. Lengths use positive `pt`, `mm`,
`cm` or `in` values. See the `md2pdf://example` MCP resource or
`tests/extended.md` for a complete rendered example.
