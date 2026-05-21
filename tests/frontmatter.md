---
title: Phase 4 — Frontmatter & Remote Images
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
authors:
  - Ada Lovelace
  - Alan Turing
---

This document's title and authors come from YAML frontmatter, parsed by
`yaml.decode` inside the Typst package.

## Remote image

The image below is fetched by the host shim's two-pass flow
(`typst query` → prefetch → `typst compile`):

![A remote placeholder image](https://placehold.co/360x140.png)

## Regular content

Everything else still works — **bold**, _italic_, and a list:

- one
- two
