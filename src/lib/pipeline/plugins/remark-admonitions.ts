import type { Node, Parent } from 'unist';
import type { Root, Html } from 'mdast';

const KINDS = new Set([
	'success',
	'warning',
	'tip',
	'info',
	'danger',
	'note',
	// Layout directives — same plumbing, different renderer. The renderer
	// dispatches on `kind` so these flow through preprocessor + remark plugin
	// unchanged and are turned into Typst `align()` / `grid()` calls.
	'left',
	'center',
	'right',
	'row'
]);

export type AdmonitionBlock = {
	kind: string;
	title: string;
	source: string;
};

export interface AdmonitionNode extends Node {
	type: 'admonition';
	kind: string;
	title: string;
	source: string;
}

/**
 * Preprocess raw markdown to find `:::kind ... :::` blocks and replace them
 * with an HTML-comment placeholder. The block contents are stored separately
 * so we can re-parse them later, recursively, with the full pipeline.
 *
 * Fence length matters (CommonMark code-fence style): an opener of N colons
 * is closed only by a line of N or more colons. So `::::row` is closed by
 * `::::` and can safely contain inner 3-colon admonitions like `:::warning`.
 * A 3-colon opener is closed by `:::` exactly as before — backward compatible.
 */
export function preprocessAdmonitions(markdown: string): {
	markdown: string;
	blocks: AdmonitionBlock[];
} {
	const blocks: AdmonitionBlock[] = [];
	const lines = markdown.split(/\r?\n/);
	const out: string[] = [];
	let i = 0;
	while (i < lines.length) {
		const line = lines[i];
		const open = /^(:{3,})\s*([A-Za-z][A-Za-z0-9_-]*)\s*(.*?)\s*$/.exec(line);
		if (open && KINDS.has(open[2].toLowerCase())) {
			const fenceLen = open[1].length;
			const kind = open[2].toLowerCase();
			const title = open[3] || '';
			const closer = new RegExp('^:{' + fenceLen + ',}\\s*$');
			const body: string[] = [];
			i++;
			while (i < lines.length && !closer.test(lines[i])) {
				body.push(lines[i]);
				i++;
			}
			i++; // skip closing fence
			const id = blocks.length;
			blocks.push({ kind, title, source: body.join('\n') });
			// Surround with blank lines so remark parses the placeholder as its own block.
			out.push('', `<!--admonition:${id}-->`, '');
			continue;
		}
		out.push(line);
		i++;
	}
	return { markdown: out.join('\n'), blocks };
}

/**
 * remark plugin: scans the mdast for the HTML-comment placeholders and swaps
 * them in-place for `admonition` nodes carrying the block metadata.
 */
export default function remarkAdmonitions(blocks: AdmonitionBlock[] = []) {
	const re = /^<!--admonition:(\d+)-->$/;
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

		function maybeReplace(node: any): AdmonitionNode | null {
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

		function makeNode(id: number): AdmonitionNode | null {
			const block = blocks[id];
			if (!block) return null;
			return {
				type: 'admonition',
				kind: block.kind,
				title: block.title,
				source: block.source
			};
		}
	};
}
