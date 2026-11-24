# Sample Wiki Page

Plain paragraph with **bold**, *italic*, __underline__, ~~strikethrough~~, ==highlight==, and `inline code`.

\

A line above consists only of a backslash (empty paragraph).

Line with a hard break\
and a continuation line.

## Heading with __underline__ inside

### Third level

:::info
Notice without a space after the colons.
:::

:::warning
Warning notice.
:::

:::tip
Tip notice.
:::

:::success
Success notice.
:::

:::info
Notice with a list:

* item one
* item two
:::

Math inline $a^2 + b^2 = c^2$ in a sentence.

$$
\begin{align*}
a &= b + c \\
d &= e
\end{align*}
$$

$$
\begin{bmatrix} 1 & 0 \\ 0 & 1 \end{bmatrix} \coloneq M, \quad \overset{def}{=}
$$

```python
def hello():
    return "world"
```

```
plain fence without language
```

 ![](https://example.invalid/a.png)

 ![Alt text](https://example.invalid/a.png "Image title")

 ![](https://example.invalid/a.png "=200x100")

 ![](https://example.invalid/a.png "Title =200x")

 ![](https://example.invalid/a.png "right-50")

* ![](https://example.invalid/a.png)

[sample-file.pdf 1874](https://wiki.example.com/api/attachments.redirect?id=sample)

[clip.mp4 640x480](https://example.com/files/clip.mp4)

[https://example.com/embed/abc](https://example.com/embed/abc)

<https://example.com/plain-url>

@[Alex Example](mention://11111111-2222-3333-4444-555555555555/user/66666666-7777-8888-9999-000000000000)

[Linked Document](/doc/sample-doc-abcdef123)

@[Card Document](/doc/sample-doc-abcdef123 "card")

Emoji shortcode :rocket: and :+1:.

| Name | Value |  |
|------|-------|--|
| a    | 1     |  |
| b    | 2     |  |

| Left | Centre | Right |
|:-----|:------:|------:|
|      | x      | y     |

| Rich | Cell |
|------|------|
| **bold** and `code` | line one<br>line two |
| - [ ] task in cell | $x^2$ |

- [ ] open task
- [x] done task
  - [ ] nested open task

***

---

> Quoted paragraph.
>
> > Nested quote.

> ### Quote holding only a heading

1. first
2. second
3. third

a. alpha item
b. beta item

+++
Toggle summary line

Hidden body text.
+++

++++
Outer toggle

+++
Inner toggle

Inner body.
+++
++++

Final paragraph with a [regular link](https://example.com "link title") and an escaped \* star and \~ tilde.
