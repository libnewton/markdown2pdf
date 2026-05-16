import { visit } from 'unist-util-visit';
import { get } from 'node-emoji';
import type { Node, Parent } from 'unist';
import type { Text, PhrasingContent } from 'mdast';

// Match :shortcode: — letters, digits, underscore, hyphen, plus.
// Anchored so we don't munch through colons used in `:::admonition` syntax;
// admonition placeholders have already been replaced by the time this runs.
const SHORTCODE_RE = /:([a-z0-9_+-]+):/gi;

export default function remarkEmojiShortcodes() {
	return (tree: Node) => {
		visit(tree, 'text', (node: Text, index: number | undefined, parent: Parent | undefined) => {
			if (index === undefined || parent === undefined) return;
			const value = node.value;
			if (!value.includes(':')) return;
			SHORTCODE_RE.lastIndex = 0;
			if (!SHORTCODE_RE.test(value)) return;

			const newChildren: PhrasingContent[] = [];
			let lastEnd = 0;
			let match: RegExpExecArray | null;
			SHORTCODE_RE.lastIndex = 0;
			let replaced = false;
			while ((match = SHORTCODE_RE.exec(value)) !== null) {
				const emoji = get(match[1]);
				if (!emoji || emoji === `:${match[1]}:`) continue; // unknown shortcode → leave literal
				if (match.index > lastEnd) {
					newChildren.push({ type: 'text', value: value.slice(lastEnd, match.index) });
				}
				newChildren.push({ type: 'text', value: emoji });
				lastEnd = match.index + match[0].length;
				replaced = true;
			}
			if (!replaced) return;
			if (lastEnd < value.length) {
				newChildren.push({ type: 'text', value: value.slice(lastEnd) });
			}
			parent.children.splice(index, 1, ...newChildren);
			return index + newChildren.length;
		});
	};
}
