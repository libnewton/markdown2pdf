import { visit } from 'unist-util-visit';
import type { Node, Parent } from 'unist';
import type { Text, PhrasingContent } from 'mdast';

// Regex to capture ^...^ or ~...~
// We use non-greedy matching.
// Capturing groups are used so split includes the separators.
// Group 1: ^...^
// Group 2: ~...~
const REGEX = /(\^.+?\^)|(~.+?~)/g;

export default function remarkSimpleSupersub() {
	return (tree: Node) => {
		visit(tree, 'text', (node: Text, index: number | undefined, parent: Parent | undefined) => {
			if (index === undefined || parent === undefined) return;

			const value = node.value;
			// Quick check if we have potential markers
			if (!value.includes('^') && !value.includes('~')) return;

			// Split preserves capturing groups.
			// Example: "a^b^c" split by /(\^.+?\^)|(~.+?~)/
			// Result: ["a", "^b^", undefined, "c"]
			const parts = value.split(REGEX);
			
			// If no split happened (length 1), nothing to do.
			if (parts.length === 1) return;

			const newChildren: PhrasingContent[] = [];

			for (const part of parts) {
				if (!part) continue; // Skip undefined from unused capturing group

				if (part.startsWith('^') && part.endsWith('^') && part.length > 2) {
					newChildren.push({
						type: 'superscript',
						children: [{ type: 'text', value: part.slice(1, -1) }]
					} as any);
				} else if (part.startsWith('~') && part.endsWith('~') && part.length > 2) {
					newChildren.push({
						type: 'subscript',
						children: [{ type: 'text', value: part.slice(1, -1) }]
					} as any);
				} else {
					// Detect if it's just a raw text similar to marker but not matched by regex?
					// Use the fact that split separates matched tokens. 
					// However, split might leave empty strings if match is at start/end.
					if (part !== '') {
						newChildren.push({ type: 'text', value: part });
					}
				}
			}

			// Replace the current text node with the new nodes
			parent.children.splice(index, 1, ...newChildren);

			// Return the new index to continue traversal after our inserted nodes
			return index + newChildren.length;
		});
	};
}
