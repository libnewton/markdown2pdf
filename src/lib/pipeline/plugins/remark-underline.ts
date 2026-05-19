import type { Node, Parent } from 'unist';
import type { PhrasingContent } from 'mdast';

// Non-standard extension: __text__ means UNDERLINE (not bold).
// Standard markdown treats __ and ** identically (both strong). To override
// that without rewriting micromark, we preprocess the raw markdown source and
// rewrite `__...__` pairs to inline `<u>...</u>` tags before remark parses.
// The remark plugin below then walks the AST, finds adjacent html('<u>') /
// html('</u>') siblings, and wraps the content between them into an
// `underline` node which markdownToTypst renders as `#underline[...]`.

export interface UnderlineNode extends Node {
	type: 'underline';
	children: PhrasingContent[];
}

const OPEN_RE = /^<u>$/i;
const CLOSE_RE = /^<\/u>$/i;

/**
 * Walk the raw markdown source and replace `__X__` with `<u>X</u>`.
 * Skips fenced code blocks (``` / ~~~) and inline code spans (`...`).
 * Honors backslash escapes (\_ stays literal).
 */
export function preprocessUnderlines(markdown: string): string {
	const lines = markdown.split('\n');
	let inFence = false;
	let fenceChar = '';
	let fenceLen = 0;
	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];
		const m = /^\s{0,3}(`{3,}|~{3,})/.exec(line);
		if (m) {
			const char = m[1][0];
			const len = m[1].length;
			if (!inFence) {
				inFence = true;
				fenceChar = char;
				fenceLen = len;
				continue;
			}
			if (char === fenceChar && len >= fenceLen) {
				inFence = false;
				fenceChar = '';
				fenceLen = 0;
				continue;
			}
		}
		if (inFence) continue;
		lines[i] = transformLine(line);
	}
	return lines.join('\n');
}

function transformLine(line: string): string {
	let out = '';
	let i = 0;
	while (i < line.length) {
		const c = line[i];
		if (c === '\\' && i + 1 < line.length) {
			out += line.slice(i, i + 2);
			i += 2;
			continue;
		}
		if (c === '`') {
			let cnt = 1;
			while (line[i + cnt] === '`') cnt++;
			const run = '`'.repeat(cnt);
			const close = line.indexOf(run, i + cnt);
			if (close !== -1) {
				out += line.slice(i, close + cnt);
				i = close + cnt;
				continue;
			}
			out += c;
			i++;
			continue;
		}
		if (c === '_' && line[i + 1] === '_') {
			const closeIdx = findCloser(line, i + 2);
			if (closeIdx !== -1 && closeIdx > i + 2) {
				const inner = line.slice(i + 2, closeIdx);
				out += `<u>${transformLine(inner)}</u>`;
				i = closeIdx + 2;
				continue;
			}
			out += '__';
			i += 2;
			continue;
		}
		out += c;
		i++;
	}
	return out;
}

function findCloser(line: string, from: number): number {
	let j = from;
	while (j < line.length - 1) {
		const c = line[j];
		if (c === '\\' && j + 1 < line.length) {
			j += 2;
			continue;
		}
		if (c === '`') {
			let cnt = 1;
			while (line[j + cnt] === '`') cnt++;
			const run = '`'.repeat(cnt);
			const close = line.indexOf(run, j + cnt);
			if (close !== -1) {
				j = close + cnt;
				continue;
			}
			j++;
			continue;
		}
		if (c === '_' && line[j + 1] === '_') return j;
		j++;
	}
	return -1;
}

export default function remarkUnderline() {
	return (tree: Node) => {
		walk(tree as Parent);
	};
}

function walk(parent: Parent) {
	if (!parent || !Array.isArray(parent.children)) return;
	for (const child of parent.children) {
		if (child && Array.isArray((child as any).children)) {
			walk(child as Parent);
		}
	}
	const children = parent.children;
	for (let i = 0; i < children.length; i++) {
		const node: any = children[i];
		if (node?.type !== 'html' || typeof node.value !== 'string') continue;
		if (!OPEN_RE.test(node.value.trim())) continue;
		let depth = 1;
		let closeIdx = -1;
		for (let j = i + 1; j < children.length; j++) {
			const n: any = children[j];
			if (n?.type !== 'html' || typeof n.value !== 'string') continue;
			const v = n.value.trim();
			if (OPEN_RE.test(v)) depth++;
			else if (CLOSE_RE.test(v)) {
				depth--;
				if (depth === 0) {
					closeIdx = j;
					break;
				}
			}
		}
		if (closeIdx === -1) continue;
		const inner = children.slice(i + 1, closeIdx) as PhrasingContent[];
		const underline: UnderlineNode = { type: 'underline', children: inner };
		children.splice(i, closeIdx - i + 1, underline as any);
	}
}
