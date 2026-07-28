---
title: Table Stress Test
subtitle: Every table shape the engine has to survive
---

## 1. Baseline shapes

Two columns, plain.

| Key | Value |
| --- | ----- |
| a   | b     |
| c   | d     |

Single column.

| Only |
| ---- |
| one  |
| two  |

Eight columns, no width markers.

| A | B | C | D | E | F | G | H |
| - | - | - | - | - | - | - | - |
| 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |

Twelve narrow columns with real words in each.

| Alpha | Bravo | Charlie | Delta | Echo | Foxtrot | Golf | Hotel | India | Juliet | Kilo | Lima |
| ----- | ----- | ------- | ----- | ---- | ------- | ---- | ----- | ----- | ------ | ---- | ---- |
| one | two | three | four | five | six | seven | eight | nine | ten | eleven | twelve |

---

## 2. Width markers

One `+` on the last column.

| Key | Value |
| --- | ---+  |
| a   | some longer value that should get twice the share |
| b   | short |

Escalating marks: 1, 2, 3, 4 shares.

| One | Two | Three | Four |
| --- | --+ | --++  | --+++ |
| x   | x   | x     | x     |

Extreme ratio: 1 vs 6.

| Tiny | Huge |
| ---- | ------+++++ |
| id   | A long descriptive sentence that needs the room it was given, and then some more words to make the line wrap at least twice inside the cell. |

All columns marked equally (should look like no markers at all).

| A | B | C |
| ---+ | ---+ | ---+ |
| 1 | 2 | 3 |

Markers combined with alignment colons.

| Left | Center | Right |
| :---+ | :---++: | ---+: |
| l | c | r |
| left aligned wide | centered and wider | right |

---

## 3. Long unbreakable content

Long URL as an autolink in a narrow column.

| ID | Link |
| -- | ---- |
| 1 | https://example.com/very/long/path/segment/that/keeps/going/and/going/index.html?query=value&another=value#fragment |
| 2 | short |

Long URL with a label.

| ID | Link |
| -- | ---- |
| 1 | [documentation for the thing](https://example.com/very/long/path/segment/that/keeps/going/and/going/index.html?query=value) |

A single very long word (no break opportunities at all).

| Word | Note |
| ---- | ---- |
| Donaudampfschifffahrtselektrizitaetenhauptbetriebswerkbauunterbeamtengesellschaft | German |
| pneumonoultramicroscopicsilicovolcanoconiosis | English |

Long file path.

| Path | Note |
| ---- | ---- |
| /usr/local/share/some/deeply/nested/directory/structure/config.default.yaml | absolute |

---

## 4. Inline code in cells

Short inline code (under the 40-cluster box threshold).

| Call | Meaning |
| ---- | ------- |
| `fn()` | invoke |
| `--flag` | option |

Long inline code (over the 40-cluster threshold, uses the breakable branch).

| Call | Meaning |
| ---- | ------- |
| `some_function_with_a_very_long_name(argument_one, argument_two)` | invoke |
| `SELECT * FROM table WHERE column = 'value' ORDER BY other_column DESC` | query |

Inline code just under and just over 40 characters, in the narrow column.

| Code | Note |
| --+ | ---- |
| `0123456789012345678901234567890123456789` | exactly 40 |
| `01234567890123456789012345678901234567890` | 41 |

Inline code containing a backtick and a pipe.

| Code | Note |
| ---- | ---- |
| ``a`b`` | backtick inside |
| `a \| b` | escaped pipe |
| `#table(columns: (1fr, 2fr))` | typst call |

Short code spans in a very narrow column (the atomic-box branch).

| Code | Description | More |
| ---- | ------------------+++ | ---- |
| `handleRequest(ctx)` | invoked per request | x |
| `--no-verify-signatures` | option | x |
| `a1b2c3d4e5f6a7b8c9d0` | commit hash | x |

Inline code next to text and markup.

| Mixed |
| ----- |
| Run `npm run build` then **check** the *output* in `web/build/`. |

---

## 5. Code blocks near and around tables

A fenced block whose content looks like a separator row with `+` markers must not be rewritten.

```markdown
| A | B |
| --- | ---++ |
| 1 | 2 |
```

Table immediately after the fence:

| After | Fence |
| ----- | ---+  |
| still | works |

Indented (four-space) code block containing a table:

    | A | B |
    | --- | ---+ |
    | 1 | 2 |

---

## 6. Ragged rows

Fewer cells than the header.

| A | B | C |
| - | - | - |
| 1 | 2 | 3 |
| 1 | 2 |
| 1 |
| 1 | 2 | 3 |

More cells than the header.

| A | B |
| - | - |
| 1 | 2 | 3 |
| 1 | 2 |

Empty cells everywhere.

| A | B | C |
| - | - | - |
|   |   |   |
| 1 |   | 3 |

Ragged rows with width markers.

| A | B | C |
| - | --+ | - |
| 1 | 2 | 3 |
| 1 | 2 |
| 1 |

---

## 7. Placement and nesting

Table without outer pipes (width markers are not supported here).

A | B
--- | ---
1 | 2

Table without outer pipes but with a `+` marker (must still render as a table):

A | B
--- | ---+
1 | 2

Table inside a blockquote:

> | Q | A |
> | - | - |
> | 1 | 2 |

Table inside a blockquote with a width marker:

> | Q | A |
> | - | --+ |
> | 1 | 2 |

Table inside a list item:

1. First step

   | Step | Detail |
   | ---- | ------ |
   | one  | do it  |

2. Second step

   | Step | Detail |
   | ---- | ---+   |
   | one  | do it with a marker |

Table inside an admonition:

:::note
| In | Note |
| -- | ---+ |
| 1  | 2    |
:::

---

## 8. Latch behaviour

A width-marked table followed immediately by an unmarked table — the second must not inherit the first table's widths.

| A | B |
| - | ----+++ |
| 1 | 2 |

| C | D |
| - | - |
| 3 | 4 |

A separator-looking row with markers that is *not* preceded by a header row (no table):

| ---+ | ---++ |

| E | F |
| - | - |
| 5 | 6 |

---

## 9. Rich cell content

Links, emphasis, strikethrough, math, footnotes, emoji.

| Kind | Example |
| ---- | ------+ |
| link | [Typst](https://typst.app) |
| bold | **strong** and *emphatic* and ~~struck~~ |
| math | $E = mc^2$ and $\sum_{i=1}^{n} i$ |
| mark | ==highlighted== text |
| emoji | 🚀 ✅ 🇩🇪 |
| under | <u>underlined</u> |

Line breaks inside cells.

| Cell | Content |
| ---- | ------- |
| br tag | line one<br>line two |
| backslash | line one\
line two |

Escaped pipes in cell text.

| Expr | Meaning |
| ---- | ------- |
| a \| b | alternation |
| \|x\| | absolute value |

---

## 10. Long tables

A table long enough to break across a page, with a width marker.

| # | Description |
| - | ---------+++ |
| 1 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 2 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 3 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 4 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 5 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 6 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 7 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 8 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 9 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 10 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 11 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 12 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 13 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 14 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 15 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 16 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 17 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 18 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 19 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 20 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 21 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 22 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 23 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 24 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 25 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 26 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 27 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 28 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 29 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |
| 30 | The quick brown fox jumps over the lazy dog and keeps running past the horizon. |

A single cell with a very large amount of text.

| Note | Body |
| ---- | -------+++ |
| long | Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. |

---

## 11. Marker edge cases

A separator cell that is only `+` after two dashes, minimum width.

| A | B |
| -- | --+ |
| 1 | 2 |

Extra whitespace around the markers.

| A | B |
|  --   |   --++   |
| 1 | 2 |

Markers on either side of the alignment colon.

| Left | Right |
| :--+ | --:+ |
| l | r |

Markers on the first column only.

| A | B |
| ---++ | --- |
| a long first column value that needs the width | b |
