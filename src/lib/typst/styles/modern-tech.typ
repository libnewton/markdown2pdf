// 方案一：现代科技风 (The "Modern Tech" Style)
// 特点：全无衬线（更像网页阅读）、段间距、无首行缩进、代码块现代风格。

#import "../admonitions.typ": admonition, spoiler, task-item, md2pdf-list-markers, md2pdf-enum-numbering

#let article(title: "", authors: (), ..args, body) = {
  let lang = args.at("lang", default: "zh")
  let page-numbers = args.at("page-numbers", default: true)
  // 1) 页面设置：宽边距，利于阅读
  set page(
    paper: "a4",
    margin: (x: 1.8cm, y: 2cm),
    numbering: if page-numbers { "1" } else { none },
    // Only show the page-number footer when there are 2+ pages.
    // A single-page document doesn't need "1" at the bottom.
    footer: if page-numbers {
      context {
        let total = counter(page).final().first()
        if total > 1 {
          align(center, text(9pt, fill: luma(120), counter(page).display("1")))
        }
      }
    },
  )
  set document(title: title, author: authors, date: none)

  // 2) 字体栈：英文/数字用高质量西文字体，中文优先无衬线（屏幕阅读更舒适），末尾添加 emoji 字体
  set text(
    font: (
      "IBM Plex Sans",
      "Roboto",
      "Libertinus Sans",
      "Noto Sans CJK SC",
      "Noto Sans SC",
      "Noto Serif SC",
      "Noto Color Emoji",
    ),
    size: 10.5pt,
    lang: lang,
  )

  // 3) 段落：放弃首行缩进，采用“段间距”模式（更接近网页阅读）
  set par(
    justify: true,
    leading: 1em,
    first-line-indent: 0pt,
    spacing: 1.2em,
  )
  set list(indent: 1em, body-indent: 0.5em, spacing: 0.8em, marker: md2pdf-list-markers)
  set enum(indent: 1em, body-indent: 0.5em, spacing: 0.8em, full: true, numbering: md2pdf-enum-numbering)

  // 4) 标题：加粗、深灰、留白（建立清晰层级）
  show heading: it => {
    set text(
      weight: "bold",
      fill: rgb("#333333"),
      font: ("IBM Plex Sans", "Roboto", "Noto Sans CJK SC", "Noto Sans SC"),
    )
    block(above: 2em, below: 1em, it)
  }

  // 5) 链接颜色：科技蓝
  show link: set text(fill: rgb("#0074de"))

  // 6) 引用块：左侧高亮线 + 浅背景
  set quote(block: true)
  show quote: it => {
    set par(first-line-indent: 0pt)
    block(
      fill: luma(248),
      stroke: (left: 2pt + rgb("#0074de")),
      inset: (left: 0.9em, right: 0.9em, top: 0.7em, bottom: 0.7em),
      radius: 6pt,
      width: 100%,
      it.body,
    )
  }

  // 7) 行内代码：轻背景 + 圆角
  // Monospace x-height runs hot vs sans-serif at the same point size, so we
  // shrink inline code to 0.85em. `outset` extends the background up/down
  // beyond the layout box so tall glyphs (brackets, descenders) sit inside
  // the tint without pushing the surrounding line apart.
  show raw.where(block: false): it => box(
    fill: luma(238),
    inset: (x: 4pt, y: 0pt),
    outset: (top: 3pt, bottom: 2pt),
    radius: 3pt,
    text(size: 0.85em, it),
  )

  // 8) 代码块：圆角 + 浅灰背景 + 左侧行号
  // Scope the raw.line rule inside the block rule so it does not fire for
  // inline `code` (which would otherwise get wrapped in a grid and break).
  show raw.where(block: true): it => block(
    fill: luma(245),
    inset: 12pt,
    radius: 6pt,
    width: 100%,
    stroke: none,
    {
      set par(leading: 0.55em, spacing: 0em, first-line-indent: 0pt, justify: false)
      show raw.line: ln => grid(
        columns: (1.6em, 1fr),
        column-gutter: 0.8em,
        align(right + top, text(fill: luma(160), size: 0.9em, str(ln.number))),
        ln.body,
      )
      it
    },
  )
  show raw: set text(font: ("JetBrains Mono", "Fira Code", "Consolas", "DejaVu Sans Mono"))

  // 9) 表格样式：浅灰表头 + 圆角边框
  set table(
    stroke: (paint: luma(200), thickness: 0.5pt),
    inset: 8pt,
    fill: (x, y) => if y == 0 { luma(240) } else { none },
  )
  show table: it => block(
    radius: 6pt,
    stroke: 0.5pt + luma(200),
    clip: true,
    inset: 0pt,
    it,
  )
  show table: set par(justify: false, spacing: 0.6em)
  show table.cell.where(y: 0): set text(weight: "bold")

  // 10) 高亮：稍柔和的黄
  show highlight: set highlight(fill: rgb("#FEF08A"))

  // 11) 图片：带 alt 时渲染为带 caption 的居中 figure
  show image: it => align(center, it)

  // 标题区（可选）
  if title != "" {
    align(center)[
      #text(1.8em, weight: "black", title)
      #if authors.len() > 0 [
        #v(0.35em)
        #text(0.95em, fill: rgb("#555555"), authors.join(", "))
      ]
    ]
    v(1em)
  }

  body
}
