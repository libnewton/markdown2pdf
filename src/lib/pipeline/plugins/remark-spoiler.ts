import type { Node, Parent } from 'unist';
import type { Root, Html } from 'mdast';

export type SpoilerBlock = {
	summary: string;
	source: string;
};

export interface SpoilerNode extends Node {
	type: 'spoiler';
	summary: string;
	source: string;
}

/**
 * Preprocess raw markdown to find +++++ ... +++++ spoiler blocks and replace
 * each with an HTML-comment placeholder. First line after the opening fence
 * (if non-blank and not part of the body) is treated as the summary.
 */
export function preprocessSpoilers(markdown: string): {
	markdown: string;
	blocks: SpoilerBlock[];
} {
	const blocks: SpoilerBlock[] = [];
	const lines = markdown.split(/\r?\n/);
	const out: string[] = [];
	let i = 0;
	while (i < lines.length) {
		const line = lines[i];
		// Tolerate up to a few leading spaces on the fence — WYSIWYG editors
		// (Milkdown) often serialize the closing `+++++` indented after a list.
		const open = /^\s{0,3}\+{5,}\s*(.*?)\s*$/.exec(line);
		if (open) {
			// Don't confuse with a setext underline or some unrelated +++++ in code.
			// Treat as spoiler only if there's a matching closing fence below.
			let close = -1;
			for (let j = i + 1; j < lines.length; j++) {
				if (/^\s{0,3}\+{5,}\s*$/.test(lines[j])) {
					close = j;
					break;
				}
			}
			if (close !== -1) {
				const inline = open[1].trim();
				const bodyStart = i + 1;
				let summary = '';
				let bodyLines = lines.slice(bodyStart, close);
				if (inline) {
					summary = inline;
				} else {
					// First non-blank line is summary, rest is body.
					let k = 0;
					while (k < bodyLines.length && bodyLines[k].trim() === '') k++;
					if (k < bodyLines.length) {
						summary = bodyLines[k].trim();
						bodyLines = bodyLines.slice(k + 1);
					}
				}
				const id = blocks.length;
				blocks.push({ summary: summary || 'spoiler', source: bodyLines.join('\n') });
				out.push('', `<!--spoiler:${id}-->`, '');
				i = close + 1;
				continue;
			}
		}
		out.push(line);
		i++;
	}
	return { markdown: out.join('\n'), blocks };
}

export default function remarkSpoiler(blocks: SpoilerBlock[] = []) {
	const re = /^<!--spoiler:(\d+)-->$/;
	return (tree: Node) => {
		if (blocks.length === 0) return;
		const root = tree as Root;
		walk(root);

		function walk(parent: Parent) {
			for (let idx = 0; idx < parent.children.length; idx++) {
				const node: any = parent.children[idx];
				const replaced = maybeReplace(node);
				if (replaced) {
					parent.children.splice(idx, 1, replaced as any);
					continue;
				}
				if (node && Array.isArray(node.children)) walk(node as Parent);
			}
		}

		function maybeReplace(node: any): SpoilerNode | null {
			if (node.type === 'html' && typeof node.value === 'string') {
				const m = re.exec(node.value.trim());
				if (m) return makeNode(Number(m[1]));
			}
			if (node.type === 'paragraph' && Array.isArray(node.children) && node.children.length === 1) {
				const first = node.children[0];
				if (first?.type === 'html' && typeof (first as Html).value === 'string') {
					const m = re.exec(((first as Html).value ?? '').trim());
					if (m) return makeNode(Number(m[1]));
				}
			}
			return null;
		}

		function makeNode(id: number): SpoilerNode | null {
			const block = blocks[id];
			if (!block) return null;
			return {
				type: 'spoiler',
				summary: block.summary,
				source: block.source
			};
		}
	};
}
