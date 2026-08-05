import type { SvgPage } from './svg-split';

const SVG_NS = 'http://www.w3.org/2000/svg';
// typst-ts puts its text-selection layer in XHTML elements under the `h5`
// prefix, so a fragment cannot be parsed without that declaration.
const SVG_OPEN =
	`<svg xmlns="${SVG_NS}" xmlns:xlink="http://www.w3.org/1999/xlink"` +
	' xmlns:h5="http://www.w3.org/1999/xhtml">';

function parseSvg(markup: string): SVGSVGElement {
	const doc = new DOMParser().parseFromString(SVG_OPEN + markup + '</svg>', 'image/svg+xml');
	scrub(doc.documentElement);
	return document.adoptNode(doc.documentElement) as unknown as SVGSVGElement;
}

/**
 * Drop anything executable before the tree is adopted into the live document.
 *
 * This SVG is typst.ts's rendering of a document we did not write, and unlike
 * the HTML preview it does not land in a shadow root — it goes straight into
 * the page. A `<script>` built by DOMParser is inert even after adoption, but
 * an `onload=` attribute is not, and neither is a `javascript:` href.
 */
function scrub(root: Element) {
	const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
	const doomed: Element[] = [];
	for (let el: Node | null = root; el; el = walker.nextNode()) {
		if (!(el instanceof Element)) continue;
		if (el.localName === 'script' || el.localName === 'foreignObject') {
			doomed.push(el);
			continue;
		}
		for (const attr of [...el.attributes]) {
			const name = attr.localName.toLowerCase();
			const scheme = attr.value.trim().slice(0, 11).toLowerCase();
			if (name.startsWith('on') || scheme.startsWith('javascript:')) {
				el.removeAttributeNode(attr);
			}
		}
	}
	doomed.forEach((el) => el.remove());
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
	const el = parseSvg(page.markup);
	el.setAttribute('viewBox', `0 0 ${page.width} ${page.height}`);
	// The page carries its offset within the full document; standalone it
	// starts at the origin.
	el.firstElementChild?.setAttribute('transform', 'translate(0, 0)');
	return el;
}
