import { visit } from 'unist-util-visit';
import type { Node, Parent } from 'unist';
import type { Text, PhrasingContent } from 'mdast';

export interface TwemojiNode extends Node {
	type: 'twemoji';
	codepoint: string; // e.g. "1f600", or "1f1fa-1f1f8" for flags
	raw: string;
}

// Matches emoji presentations. Covers BMP miscellaneous symbols, dingbats,
// supplementary symbols & pictographs, plus regional-indicator flag pairs.
// Anchors to handle surrogate pairs and ZWJ sequences greedily.
const EMOJI_REGEX =
	/(?:\p{Extended_Pictographic}(?:‍\p{Extended_Pictographic})*(?:️)?|\p{Regional_Indicator}{2})/gu;

/**
 * Convert a JS string emoji into the twemoji filename codepoint (lowercase,
 * dash-joined, with U+FE0F variation selectors stripped except when the
 * emoji is a single bare codepoint that lives at a -fe0f filename).
 */
function toCodepoint(emoji: string): string {
	const parts: string[] = [];
	for (const ch of emoji) {
		const cp = ch.codePointAt(0);
		if (cp === undefined) continue;
		// Strip U+FE0F (variation selector) — twemoji filenames drop it except for ~12 exceptions we accept fallback for.
		if (cp === 0xfe0f) continue;
		parts.push(cp.toString(16));
	}
	return parts.join('-');
}

export default function remarkTwemoji() {
	return (tree: Node) => {
		visit(tree, 'text', (node: Text, index: number | undefined, parent: Parent | undefined) => {
			if (index === undefined || parent === undefined) return;
			const value = node.value;
			EMOJI_REGEX.lastIndex = 0;
			if (!EMOJI_REGEX.test(value)) return;

			const newChildren: PhrasingContent[] = [];
			let lastEnd = 0;
			let match: RegExpExecArray | null;
			EMOJI_REGEX.lastIndex = 0;
			while ((match = EMOJI_REGEX.exec(value)) !== null) {
				if (match.index > lastEnd) {
					newChildren.push({ type: 'text', value: value.slice(lastEnd, match.index) });
				}
				const raw = match[0];
				const codepoint = toCodepoint(raw);
				newChildren.push({ type: 'twemoji', codepoint, raw } as any);
				lastEnd = match.index + raw.length;
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
