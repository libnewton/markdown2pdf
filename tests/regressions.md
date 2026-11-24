---
title: 123
lang: de_DE
header-left: 5
header-right: "[Project site](https://example.org) · {title}"
footer-left: true
footer-right: "{page} / {pages}"
header-height: 12XX
page-numbers: no
cover: false
image-width: 60%
---

# Regressions

Each line below once aborted the PDF, leaked syntax or lost text.

## Text that looks like Typst

**bold**.next, _it_.com, $x$.y, :smile:.z and **b**(x) stay literal.
A comment needs two slashes: foo // bar /* baz.
= not a heading
/ not a term: list
2. not an enum item

## Math the PDF cannot typeset

Broken: $\frac{a$, unsupported: $\href{https://example.org}{x}$, a label: $\text{a<b}$.
Colon-equals: $x \coloneq y$ and $a \eqqcolon b$.
Bars: $|K|$, $K[i \bmod |K|]$ and $x |K| y$ are delimiters; $a | b$ stays a relation.

## Emoji

Joined: 🤷‍♀️ 🏴‍☠️ 👩🏽‍💻 👍🏽 :pirate_flag: — unknown flag: 🇦🇦
Text presentation: © ® ™ ↔ ✔ ▶, emoji presentation: ✔️ ▶️

## Lists

1. first line
   wraps here
2. second
3. third
   also wraps

57. starts at fifty-seven
58. and continues

- [ ] parent stays open
  - [x] checked child

a. lettered
b. list

## Inline

2^10 and 3^4 stay plain, ~5 to ~10 keep their tildes; 19^th^ and H~2~O do not.
++Inserted++, <kbd>Ctrl</kbd>+<kbd>C</kbd>, <sup>sup</sup>, <mark>mark</mark>, <span>escaped</span><!-- hidden -->.
An inline note^[Written in place.] and a link without a target: [text]().
Images without a loadable target: ![empty]() ![inline](data:image/png;base64,AAAA)

## Code fences

```js=101
console.log("numbered from 1");
```

```c++
int main() {}
```

```python title="x.py"
print("x")
```

## Blocks

:::info
```py
:::
print("still inside")
```
A note referenced from a box[^outer].
:::

[^outer]: Defined outside the box.

:::warning Never closed
This line is ordinary text.

<!--admonition:0-->

| | |
|---|---|
| empty | header |
