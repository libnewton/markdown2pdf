import type { Node, Parent } from 'unist';
import type { Root, Html, Table } from 'mdast';

// Non-standard extension: append `+` to a GFM separator cell to widen that
// column. Count of `+`s + 1 = `Nfr` in the emitted Typst `columns:` arg.
//   `---`   → 1fr  (current default; behavior unchanged)
//   `---+`  → 2fr
//   `---++` → 3fr
// Compatible with the GFM alignment colons (e.g. `:---+:`).
//
// remark-gfm parses the separator strictly, so we preprocess the raw source
// to strip the `+`s and emit a `<!--tablewidths:N-->` placeholder before the
// header row. A remark plugin then attaches the recorded widths to the parsed
// Table node's `data` so `renderTable` can read them.

type WidthsBlock = number[];

const PLACEHOLDER_RE = /^<!--tablewidths:(\d+)-->$/;
const PIPE_ROW_RE = /^\s*\|.*\|\s*$/;

// A separator cell: optional left colon, ≥2 dashes, optional `+`s, optional
// right colon. (We tolerate cells with just `+`s after dashes; anything else
// disqualifies the line as a separator.)
const SEP_CELL_RE = /^\s*:?-{2,}\+*:?\s*$/;

export function preprocessTableWidths(markdown: string): {
	markdown: string;
	blocks: WidthsBlock[];
} {
	const blocks: WidthsBlock[] = [];
	const lines = markdown.split('\n');
	const out: string[] = [];

	let inFence = false;
	let fenceChar = '';
	let fenceLen = 0;

	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];

		const fenceMatch = /^\s{0,3}(`{3,}|~{3,})/.exec(line);
		if (fenceMatch) {
			const char = fenceMatch[1][0];
			const len = fenceMatch[1].length;
			if (!inFence) {
				inFence = true;
				fenceChar = char;
				fenceLen = len;
			} else if (char === fenceChar && len >= fenceLen) {
				inFence = false;
				fenceChar = '';
				fenceLen = 0;
			}
			out.push(line);
			continue;
		}
		if (inFence) {
			out.push(line);
			continue;
		}

		const widths = parseSeparatorWidths(line, lines[i - 1]);
		if (widths) {
			const id = blocks.length;
			blocks.push(widths.widths);
			// Insert placeholder before the header row that's already in `out`.
			// `out` currently ends with the header row at index out.length - 1.
			const headerIdx = out.length - 1;
			const header = out[headerIdx];
			out.splice(headerIdx, 1, '', `<!--tablewidths:${id}-->`, '', header);
			out.push(widths.strippedSeparator);
			continue;
		}

		out.push(line);
	}

	return { markdown: out.join('\n'), blocks };
}

function parseSeparatorWidths(
	line: string,
	prevLine: string | undefined
): { widths: number[]; strippedSeparator: string } | null {
	if (!PIPE_ROW_RE.test(line)) return null;
	if (!prevLine || !PIPE_ROW_RE.test(prevLine)) return null;
	// Don't treat the previous line as a header if it's itself a separator.
	if (splitCells(prevLine).every((c) => SEP_CELL_RE.test(c))) return null;

	const cells = splitCells(line);
	if (cells.length === 0) return null;
	if (!cells.every((c) => SEP_CELL_RE.test(c))) return null;
	if (!cells.some((c) => c.includes('+'))) return null;

	const widths = cells.map((c) => 1 + (c.match(/\+/g)?.length ?? 0));
	const strippedCells = cells.map((c) => c.replace(/\+/g, ''));
	const strippedSeparator = rebuildRow(line, strippedCells);
	return { widths, strippedSeparator };
}

function splitCells(line: string): string[] {
	// Split on `|`, trim the leading/trailing empty cells that result from
	// the surrounding pipes.
	const parts = line.split('|');
	if (parts.length >= 2 && parts[0].trim() === '') parts.shift();
	if (parts.length >= 1 && parts[parts.length - 1].trim() === '') parts.pop();
	return parts;
}

function rebuildRow(original: string, cells: string[]): string {
	// Preserve leading whitespace and surrounding pipes from the original.
	const leading = /^\s*/.exec(original)?.[0] ?? '';
	const trailing = /\s*$/.exec(original)?.[0] ?? '';
	const hasLeftPipe = original.trimStart().startsWith('|');
	const hasRightPipe = original.trimEnd().endsWith('|');
	const body = cells.join('|');
	return `${leading}${hasLeftPipe ? '|' : ''}${body}${hasRightPipe ? '|' : ''}${trailing}`;
}

export default function remarkTableWidths(blocks: WidthsBlock[] = []) {
	return (tree: Node) => {
		if (blocks.length === 0) return;
		walk(tree as Root);
	};

	function walk(parent: Parent) {
		const children = parent.children;
		for (let i = 0; i < children.length; i++) {
			const node: any = children[i];
			const id = extractPlaceholderId(node);
			if (id !== null) {
				const widths = blocks[id];
				const nextIdx = findNextTableIdx(children, i + 1);
				if (nextIdx !== -1 && widths) {
					const table = children[nextIdx] as Table & { data?: any };
					table.data = { ...(table.data ?? {}), tableWidths: widths };
				}
				children.splice(i, 1);
				i--;
				continue;
			}
			if (node && Array.isArray(node.children)) walk(node as Parent);
		}
	}
}

function extractPlaceholderId(node: any): number | null {
	if (!node) return null;
	if (node.type === 'html' && typeof node.value === 'string') {
		const m = PLACEHOLDER_RE.exec(node.value.trim());
		if (m) return Number(m[1]);
	}
	if (
		node.type === 'paragraph' &&
		Array.isArray(node.children) &&
		node.children.length === 1
	) {
		const first = node.children[0];
		if (first?.type === 'html' && typeof (first as Html).value === 'string') {
			const m = PLACEHOLDER_RE.exec(((first as Html).value ?? '').trim());
			if (m) return Number(m[1]);
		}
	}
	return null;
}

function findNextTableIdx(children: any[], from: number): number {
	for (let j = from; j < children.length; j++) {
		if (children[j]?.type === 'table') return j;
	}
	return -1;
}
