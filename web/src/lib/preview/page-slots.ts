import { buildHeadElement, buildPageElement } from '$lib/typst/svg-utils';
import type { SvgDocument, SvgPage } from '$lib/typst/svg-split';

/**
 * Mount only the pages near the viewport.
 *
 * A page is ~6k nodes, so a 40-page document rebuilt in full is ~240k nodes on
 * every compile. Each page gets a placeholder holding its exact aspect ratio —
 * so scroll position and document height never jump — and an
 * `IntersectionObserver` fills it in as it comes near the viewport and empties
 * it again as it leaves. The per-compile cost is then proportional to what is
 * on screen rather than to the document's length.
 */
export function pageSlots(container: HTMLDivElement) {
	const slots: HTMLDivElement[] = [];
	const visible = new Set<number>();
	let pages: SvgPage[] = [];
	let head: SVGSVGElement | null = null;

	const mount = (index: number) => {
		const slot = slots[index];
		const page = pages[index];
		if (slot && page) slot.replaceChildren(buildPageElement(page));
	};

	const observer = new IntersectionObserver(
		(entries) => {
			for (const entry of entries) {
				const index = Number((entry.target as HTMLElement).dataset.page);
				if (entry.isIntersecting) {
					visible.add(index);
					if (!entry.target.firstChild) mount(index);
				} else {
					visible.delete(index);
					entry.target.replaceChildren();
				}
			}
		},
		// A screenful ahead, so scrolling finds pages already rendered.
		{ root: container, rootMargin: '100% 0px' },
	);

	return {
		/** Show `doc`, reusing the slots already on screen where it can. */
		show(doc: SvgDocument) {
			pages = doc.pages;

			const nextHead = buildHeadElement(doc.head);
			if (head?.parentNode === container) {
				container.replaceChild(nextHead, head);
			} else {
				// The container was emptied from outside; the slots went with it.
				container.replaceChildren(nextHead);
				slots.length = 0;
				visible.clear();
			}
			head = nextHead;

			while (slots.length > doc.pages.length) {
				const slot = slots.pop()!;
				observer.unobserve(slot);
				visible.delete(slots.length);
				slot.remove();
			}
			for (let i = 0; i < doc.pages.length; i++) {
				let slot = slots[i];
				if (!slot) {
					slot = document.createElement('div');
					slot.className = 'page-slot';
					slot.dataset.page = String(i);
					container.appendChild(slot);
					slots[i] = slot;
					observer.observe(slot);
				}
				slot.style.aspectRatio = `${doc.pages[i].width} / ${doc.pages[i].height}`;
			}

			for (const index of visible) mount(index);
		},

		destroy() {
			observer.disconnect();
		},
	};
}
