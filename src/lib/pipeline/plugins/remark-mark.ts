import { visit } from 'unist-util-visit';
import type { Node, Parent } from 'unist';
import type { Text, PhrasingContent } from 'mdast';

// Match ==highlighted text==. Two equals on each side, non-greedy inner.
// Avoid matching at start/middle of `===` (Markdown setext-style underlines or `===` literals).
const REGEX = /==([^=].*?[^=]|[^=])==/g;

export default function remarkMark() {
	return (tree: Node) => {
		visit(tree, 'text', (node: Text, index: number | undefined, parent: Parent | undefined) => {
			if (index === undefined || parent === undefined) return;
			const value = node.value;
			if (!value.includes('==')) return;

			const newChildren: PhrasingContent[] = [];
			let lastEnd = 0;
			let match: RegExpExecArray | null;
			REGEX.lastIndex = 0;
			while ((match = REGEX.exec(value)) !== null) {
				if (match.index > lastEnd) {
					newChildren.push({ type: 'text', value: value.slice(lastEnd, match.index) });
				}
				newChildren.push({
					type: 'mark',
					children: [{ type: 'text', value: match[1] }]
				} as any);
				lastEnd = match.index + match[0].length;
			}
			if (lastEnd === 0) return;
			if (lastEnd < value.length) {
				newChildren.push({ type: 'text', value: value.slice(lastEnd) });
			}

			parent.children.splice(index, 1, ...newChildren);
			return index + newChildren.length;
		});
	};
}
