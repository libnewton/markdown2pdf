---
title: Quantus Dilithium HD Wallet
authors:
  - Ada Lovelace
  - Alan Turing
date: 2026-07-25
cover: arcs
cover-color: ocean
cover-subtitle: Security Assessment
cover-logo: "![](https://placehold.co/240x80.png)"
header-left: "{title}"
header-right: "{date}"
footer-left: Confidential
pageNumbers: "1/1"
---

The cover page is page one; it carries no header or footer. This paragraph is on
page two, which is where the running header and the `2 / N` footer start.

## Furniture

- `header-left` / `header-center` / `header-right`
- `footer-left` / `footer-center` / `footer-right`

Each slot holds either free text with `{page}`, `{pages}`, `{title}`,
`{subtitle}`, `{author}`, `{authors}` and `{date}` placeholders, or a Markdown
image — local or remote, resolved through the same prefetch path as body
images.

Footnotes still sit above the footer without colliding[^fn].

[^fn]: This note is rendered by Typst above the running footer.

[[pagebreak]]

## Page three

`pageNumbers: "1/1"` appends the total, so this page reads `3 / 3`. The total
counts the cover.
