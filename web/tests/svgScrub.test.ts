// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { buildPageElement } from '../src/lib/typst/svg-utils';

/**
 * The page SVG is typst.ts's rendering of a document we did not write, and it
 * is adopted into the live page rather than a shadow root — so anything
 * executable has to be gone before it lands.
 */
function page(markup: string) {
	return buildPageElement({ markup, width: 100, height: 100 });
}

describe('page SVG scrubbing', () => {
	it('drops event-handler attributes wherever they sit', () => {
		const el = page(
			'<g onload="alert(1)"><rect onclick="alert(1)" fill="red"/>' +
				'<text ONMOUSEOVER="alert(1)">x</text></g>'
		);
		expect(el.outerHTML).not.toMatch(/on[a-z]+=/i);
		// Everything that was not executable survives untouched.
		expect(el.querySelector('rect')?.getAttribute('fill')).toBe('red');
		expect(el.textContent).toContain('x');
	});

	it('drops javascript: hrefs but keeps ordinary ones', () => {
		const el = page(
			'<a href="javascript:alert(1)">a</a>' +
				'<a xlink:href="  JavaScript:alert(1)">b</a>' +
				'<a href="https://example.com/ok">c</a>' +
				'<use href="#glyph-1"/>'
		);
		expect(el.outerHTML.toLowerCase()).not.toContain('javascript:');
		expect(el.outerHTML).toContain('https://example.com/ok');
		expect(el.querySelector('use')?.getAttribute('href')).toBe('#glyph-1');
	});

	it('removes script and foreignObject entirely', () => {
		const el = page(
			'<script>alert(1)</script>' +
				'<foreignObject><div onclick="alert(1)">PAYLOAD</div></foreignObject>' +
				'<g id="keep"/>'
		);
		expect(el.querySelector('script')).toBeNull();
		expect(el.querySelector('foreignObject')).toBeNull();
		// The subtree goes with the element, not just the element.
		expect(el.outerHTML).not.toContain('PAYLOAD');
		expect(el.querySelector('#keep')).not.toBeNull();
	});

	it('scrubs nested payloads, not just the top level', () => {
		const el = page('<g><g><g><circle onload="alert(1)" r="1"/></g></g></g>');
		expect(el.outerHTML).not.toContain('onload');
		expect(el.querySelector('circle')?.getAttribute('r')).toBe('1');
	});
});
