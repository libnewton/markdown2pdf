import type { Template } from './pdf-templates';

export const CARD_TEMPLATES: Template[] = [
	{
		id: 'quick-start',
		name: 'Quick Start',
		icon: '📖',
		content: `# Social Cards Quick Start

Create beautiful social media cards with Markdown

[[pagebreak]]

## How to Paginate

Use \`[[pagebreak]]\` to split content into separate cards.

Keep each card concise. White space is your friend.

[[pagebreak]]

## Supported Formatting

**Bold**, *italic*, ~~strikethrough~~, \`inline code\`

- Unordered lists
- With nesting support
  - Like this

1. Ordered lists
2. Work too

> Blockquotes are great for highlighting key points

[[pagebreak]]

## Code & Math

Code blocks with syntax highlighting:

\`\`\`python
print("Hello, md2pdf!")
\`\`\`

Math uses LaTeX syntax:

Inline $E = mc^2$, or display math:

$$
\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}
$$

[[pagebreak]]

## Tables

| Feature | Supported |
|---------|-----------|
| Basic Markdown | ✓ |
| GFM Extensions | ✓ |
| LaTeX Math | ✓ |
| Syntax Highlighting | ✓ |

[[pagebreak]]

## Tips

1. The first card is your "cover" — keep it title-only
2. Keep each card to one screen of content
3. Use headings, lists, and quotes to structure content
4. Switch card styles in the right panel

**Delete this and start creating!**
`
	},
	{
		id: 'knowledge',
		name: 'Knowledge Share',
		icon: '💡',
		content: `# 5 TypeScript Tips You Might Not Know

Level up your code quality ✨

[[pagebreak]]

## 1. Use satisfies Instead of Type Assertion

\`\`\`typescript
const config = {
  port: 3000,
  host: "localhost"
} satisfies Config;
\`\`\`

Safer than \`as Config\` — preserves literal types!

[[pagebreak]]

## 2. Template Literal Types

\`\`\`typescript
type Route = \`/api/\${string}\`;
\`\`\`

Catch invalid paths at compile time 🎯

[[pagebreak]]

## 3. Record Over any

> **Never use any!**
> Record<string, unknown> is the safer choice.

[[pagebreak]]

## Summary

- \`satisfies\` over \`as\`
- Template literals over manual strings
- \`Record\` over \`any\`

**Follow for more coding tips!**
`
	}
];
