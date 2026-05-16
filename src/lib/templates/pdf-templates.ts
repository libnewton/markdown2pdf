export interface Template {
	id: string;
	name: string;
	icon: string;
	content: string;
}

const date = new Date().toISOString().split('T')[0];

const WELCOME = `---
lang: en
title: md2pdf Feature Demo
authors:
  - md2pdf Team
date: ${date}
pageNumbers: true
---

# Welcome — every feature, on one page

md2pdf turns Markdown into a typeset PDF, 100% in your browser. This document
exercises every syntax the renderer supports — use it as a reference.

==This sentence is highlighted== to draw the eye. **Bold**, _italic_,
~~strikethrough~~, \`inline code\`, super^script^ and sub~script~ all work
inline. Footnotes too[^demo].

[^demo]: Footnotes render as numbered notes at the foot of the page.

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

| Feature       | Supported | Notes                                  |
| ------------- | --------- | -------------------------------------- |
| GFM tables    | yes       | left/right/center alignment            |
| Headers       | yes       | light grey background, rounded corners |
| Inline markup | yes       | **bold**, \`code\`, [links](#)           |

| Left   |  Center  |  Right |
| :----- | :------: | -----: |
| left   |  middle  |  right |
| col    |   col    |    col |

---

## Code blocks (with line numbers)

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

\`\`\`bash
# Shell
npm install
npm run dev
\`\`\`

---

## Math

Inline math like $E = mc^2$ flows with the paragraph. Complex inline:
$\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}$.

Block math gets its own line:

$$
\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}
$$

$$
\\sum_{n=1}^{\\infty} \\frac{1}{n^2} = \\frac{\\pi^2}{6}
$$

---

## Quotes & callouts

> A standard Markdown blockquote. It can contain **inline formatting** and
> even \`inline code\`. The left rule and tinted background come from the
> theme — no extra syntax required.

### Themed admonitions

:::success
**Looks good.** Use \`:::success\` for confirmations, completed steps, or
positive results.
:::

:::warning
**Heads-up.** Use \`:::warning\` for caveats and things that might bite.
:::

:::tip
**Pro tip.** Use \`:::tip\` for advice or shortcuts. Inline math and \`code\`
both work inside admonitions.
:::

:::info
**For your information.** Use \`:::info\` for neutral context.
:::

:::danger
**Do not do this.** Use \`:::danger\` for destructive or dangerous actions.
:::

### Spoiler

+++++
Click to reveal

The body of a spoiler is indented under the summary line. You can write
**any markdown** inside, including \`code\` and lists:

- one
- two
- three
+++++

---

## Images

Local images, remote images, captions, and explicit dimensions are all
supported. Images are centered automatically; if you supply alt text, it
becomes a small caption beneath the image.

![Octocat — fetched live from GitHub, sized 200×200](https://octodex.github.com/images/minion.png =200x200)

You can also size without a caption, or include a remote image at its
natural width.

---

## Emojis

Unicode emoji are rendered as Twemoji SVGs (offline, bundled): 😀 🚀 🔥 ✨ 🎉 📄 🛡️.

Shortcodes work too: :smile: :heart: :rocket: :tada: :sparkles: :warning: :white_check_mark:.

---

## Links & references

Inline: [md2pdf README](https://github.com/libnewton/markdown2pdf).

Reference style: [SvelteKit][sk] and [Typst][typst] power the rendering.

[sk]: https://kit.svelte.dev
[typst]: https://typst.app

---

## Diagrams (Mermaid)

\`\`\`mermaid
graph LR
    Markdown-->Typst
    Typst-->SVG
    Typst-->PDF
\`\`\`

---

## Frontmatter & controls

The YAML block at the top of this document sets:

- \`title:\` — appears centered at the top of page one
- \`authors:\` — listed under the title
- \`date:\` — surfaced for your own use
- \`pageNumbers: true\` — toggle off to suppress footer numbers; the menu
  toggle is a default, frontmatter wins

---

## Page breaks

Use \`---\` for a horizontal rule, or the explicit token \`[[pagebreak]]\` to
start a new page right here:

[[pagebreak]]

## You are now on a new page

Anything after the token continues on the next page. Combine with sections to
keep chapters cleanly separated.

> **One more tip**: Press <kbd>Ctrl</kbd>+<kbd>Enter</kbd> in the editor to
> trigger a compile immediately, even when live preview is paused.
`;

const RESUME = `---
lang: en
title: Resume
---

# Jane Smith

📧 jane@email.com · 📱 (555) 123-4567 · 🔗 github.com/janesmith

---

## Experience

### Senior Frontend Engineer · ABC Tech Inc.
*Mar 2021 - Present*

- Led frontend architecture migration, reducing build times by 60%
- Designed component library with 50+ components, 90% team adoption
- Drove full TypeScript migration, reducing production bugs by 35%

### Frontend Developer · XYZ Internet Co.
*Jul 2019 - Feb 2021*

- Built e-commerce H5 pages serving 500K+ daily page views
- Optimized first contentful paint from 3.2s to 1.1s

## Education

### B.S. Computer Science · State University
*Sep 2015 - Jun 2019*

## Skills

| Category | Skills |
| :--- | :--- |
| Languages | TypeScript, JavaScript, HTML, CSS |
| Frameworks | React, Vue, Svelte, Next.js |
| Tools | Git, Docker, Webpack, Vite |
`;

const AI_CHAT = `---
lang: en
title: AI Chat Notes
date: ${date}
---

# AI Conversation Log

## Q: Implement quicksort in Python

\`\`\`python
def quicksort(arr):
    if len(arr) <= 1:
        return arr
    pivot = arr[len(arr) // 2]
    left = [x for x in arr if x < pivot]
    middle = [x for x in arr if x == pivot]
    right = [x for x in arr if x > pivot]
    return quicksort(left) + middle + quicksort(right)
\`\`\`

> Time complexity: Average $O(n \\log n)$, Worst $O(n^2)$

## Q: Quicksort vs Merge Sort?

| Feature | Quicksort | Merge Sort |
| :--- | :--- | :--- |
| Avg Time | $O(n \\log n)$ | $O(n \\log n)$ |
| Worst Time | $O(n^2)$ | $O(n \\log n)$ |
| Space | $O(\\log n)$ | $O(n)$ |
| Stable | No | Yes |

---

*Paste your AI conversation here, replacing the sample content above.*
`;

const NOTION = `---
lang: en
title: Notion Notes
date: ${date}
---

# Notion Export Cleanup

> 💡 **How to use**: Export from Notion as Markdown, then paste the .md file content here.

## Project Overview

| Task ID | Description | Owner | Priority | Due Date | Status | Notes |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| TASK-001 | Homepage Redesign | Alice | P0 | 2024-10-01 | In Progress | Waiting for design sign-off |
| TASK-002 | API Optimization | Bob | P1 | 2024-10-15 | Not Started | Depends on backend migration |
| TASK-003 | Feedback System | Charlie | P2 | 2024-11-01 | Done | Live, monitoring |

## Meeting Notes

### 2024-09-25 Weekly Sync

**Attendees**: Alice, Bob, Charlie

**Discussion**:
1. Homepage redesign — designs approved, dev starts next week
2. API latency — P99 over budget, need slow query investigation
3. Q4 planning — focus on UX improvements

**Action Items**:
- [ ] Alice: Finalize homepage tech spec
- [ ] Bob: Deliver API performance report
- [ ] Charlie: Compile Top 10 user feedback items

> 📝 **Next meeting**: 2024-10-02

---

*Paste your Notion export content here, replacing the sample above.*
`;

export const PDF_TEMPLATES: Template[] = [
	{ id: 'welcome', name: 'Get Started', icon: '🚀', content: WELCOME },
	{
		id: 'techDoc',
		name: 'Technical Spec',
		icon: '📝',
		content: `---\ntitle: Technical Design Document\ndate: ${date}\n---\n\n# Overview\n\n## Context\n\n## Proposed Solution\n\n## Rollout Plan\n`
	},
	{
		id: 'weeklyReport',
		name: 'Weekly Report',
		icon: '📊',
		content: `# Weekly Report - ${date}\n\n## Accomplishments\n\n## Plans for Next Week\n\n## Blockers\n`
	},
	{ id: 'resume', name: 'Resume', icon: '👤', content: RESUME },
	{ id: 'aiChat', name: 'AI Chat Notes', icon: '🤖', content: AI_CHAT },
	{ id: 'notion', name: 'Notion Notes', icon: '📋', content: NOTION }
];
