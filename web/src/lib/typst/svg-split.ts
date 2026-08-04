/**
 * Slice a composite typst-ts SVG into its shared header and per-page markup.
 *
 * The renderer returns the whole document as one string — ~14 MB for a
 * 40-page document, of which the shared `<defs>` (glyph outlines, clip paths)
 * is under 1%. Splitting is pure string work so it can run in the worker, and
 * pages stay as text until something needs to display them.
 */
export type SvgPage = {
	/** The page's `<g class="typst-page">` subtree, still as text. */
	markup: string;
	width: number;
	height: number;
};

export type SvgDocument = {
	/** Shared `<style>`/`<defs>` markup, referenced by every page. */
	head: string;
	pages: SvgPage[];
};

const PAGE_MARKER = '<g class="typst-page"';

function attrNumber(markup: string, name: string, fallback: number): number {
	const match = markup.slice(0, 400).match(new RegExp(`${name}="([\\d.]+)"`));
	return match ? Number(match[1]) : fallback;
}

export function splitSvgDocument(svg: string): SvgDocument {
	const starts: number[] = [];
	for (let i = svg.indexOf(PAGE_MARKER); i !== -1; i = svg.indexOf(PAGE_MARKER, i + 1)) {
		starts.push(i);
	}
	if (starts.length === 0) return { head: '', pages: [] };

	// Everything between the root <svg> open tag and the first page: the
	// shared style and defs. The open tag itself is dropped — the head and
	// each page get their own when they are parsed.
	const head = svg.slice(svg.indexOf('>') + 1, starts[0]);
	const end = svg.lastIndexOf('</svg>');

	const pages = starts.map((start, i) => {
		const markup = svg.slice(start, starts[i + 1] ?? (end === -1 ? svg.length : end));
		return {
			markup,
			width: attrNumber(markup, 'data-page-width', 595),
			height: attrNumber(markup, 'data-page-height', 842)
		};
	});

	return { head, pages };
}

export function svgPagesDocument(pages: string[]): SvgDocument {
	return {
		head: '',
		pages: pages.map((markup) => ({
			markup,
			width: attrNumber(markup, 'width', 595),
			height: attrNumber(markup, 'height', 842)
		}))
	};
}
