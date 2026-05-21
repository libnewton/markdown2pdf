import type { Template } from './pdf-templates';

export const SLIDES_TEMPLATES: Template[] = [
	{
		id: 'quick-start',
		name: 'Quick Start',
		icon: '📖',
		content: `---
title: Slides Mode Quick Start
authors:
  - md2pdf
---

## Basic Structure

Use \`[[pagebreak]]\` to separate slides

Set title and authors in YAML frontmatter at the top:

\`\`\`yaml
---
title: Your Presentation Title
authors:
  - Author Name
---
\`\`\`

A title slide is auto-generated when \`title\` is set

[[pagebreak]]

## Text Formatting

Full Markdown formatting is supported:

**Bold**, *italic*, ~~strikethrough~~, \`inline code\`

- Unordered lists
- With nesting
  - Like this

1. Ordered lists
2. Auto-numbered

> Blockquotes work great for key takeaways

[[pagebreak]]

## Code Blocks

Syntax highlighting is built in:

\`\`\`python
def fibonacci(n):
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a
\`\`\`

Use backticks for inline code: \`console.log("hello")\`

[[pagebreak]]

## Math

Write math using LaTeX syntax.

Inline: $E = mc^2$

Display block:

$$
\\int_{-\\infty}^{\\infty} e^{-x^2} dx = \\sqrt{\\pi}
$$

[[pagebreak]]

## Tables

| Syntax | Supported |
|--------|-----------|
| \`[[pagebreak]]\` | ✓ |
| YAML frontmatter | ✓ |
| GFM tables | ✓ |
| LaTeX math | ✓ |
| Syntax highlighting | ✓ |
| Links & images | ✓ |

[[pagebreak]]

## Tips

1. Keep each slide focused — white space helps your audience
2. Use heading levels: \`##\` for slide titles, \`###\` for sections
3. Switch slide themes in the right panel

**Delete this and start building your presentation!**
`
	},
	{
		id: 'tech-talk',
		name: 'Tech Talk',
		icon: '🎤',
		content: `---
title: "TypeScript 5.0: What's New"
authors:
  - Developer
---

## The satisfies Keyword

A safer alternative to type assertions:

\`\`\`typescript
const config = {
  port: 3000,
  host: "localhost"
} satisfies ServerConfig;
\`\`\`

- Preserves literal type inference
- Compile-time type compatibility check
- No type information loss

[[pagebreak]]

## Decorators (Stable)

ES standard decorators are finally stable:

\`\`\`typescript
function log(target: any, context: ClassMethodDecoratorContext) {
  return function (...args: any[]) {
    console.log(\`Calling \${String(context.name)}\`);
    return target.apply(this, args);
  };
}
\`\`\`

[[pagebreak]]

## const Type Parameters

\`\`\`typescript
function routes<const T extends string[]>(paths: T) {
  return paths;
}

// Type is readonly ["/home", "/about"]
const r = routes(["/home", "/about"]);
\`\`\`

[[pagebreak]]

## Summary

- **satisfies** — Safer type checking
- **Decorators** — Standardized metaprogramming
- **const type params** — Precise literal inference

> Upgrade to TypeScript 5.0 for a better developer experience!
`
	}
];
