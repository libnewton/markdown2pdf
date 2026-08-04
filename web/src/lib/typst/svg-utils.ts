import type { SvgPage } from './svg-split';

const SVG_NS = 'http://www.w3.org/2000/svg';
// typst-ts puts its text-selection layer in XHTML elements under the `h5`
// prefix, so a fragment cannot be parsed without that declaration.
const SVG_OPEN =
	`<svg xmlns="${SVG_NS}" xmlns:xlink="http://www.w3.org/1999/xlink"` +
	' xmlns:h5="http://www.w3.org/1999/xhtml">';

function parseSvg(markup: string): SVGSVGElement {
	const doc = new DOMParser().parseFromString(SVG_OPEN + markup + '</svg>', 'image/svg+xml');
	return document.adoptNode(doc.documentElement) as unknown as SVGSVGElement;
}

/**
 * The document's shared definitions, as one zero-size element. Pages resolve
 * their `<use href="#…">` against it because they share a document — so it is
 * built once instead of being copied into every page.
 */
export function buildHeadElement(head: string): SVGSVGElement {
	const el = parseSvg(head);
	el.setAttribute('class', 'typst-defs');
	el.setAttribute('aria-hidden', 'true');
	return el;
}

export function buildPageElement(page: SvgPage): SVGSVGElement {
	if (page.markup.trimStart().startsWith('<svg')) {
		const doc = new DOMParser().parseFromString(page.markup, 'image/svg+xml');
		const el = document.adoptNode(doc.documentElement) as unknown as SVGSVGElement;
		if (!el.hasAttribute('viewBox')) el.setAttribute('viewBox', `0 0 ${page.width} ${page.height}`);
		return el;
	}
	const el = parseSvg(page.markup);
	el.setAttribute('viewBox', `0 0 ${page.width} ${page.height}`);
	// The page carries its offset within the full document; standalone it
	// starts at the origin.
	el.firstElementChild?.setAttribute('transform', 'translate(0, 0)');
	return el;
}
