<script lang="ts">
	// Pageless HTML view of the document.
	//
	// The markup and its stylesheet come from the engine — byte for byte the
	// same thing the download and the CLI produce. It is mounted in a shadow
	// root so the document's CSS and the app's CSS can never reach each other.

	let { html = '', theme }: { html?: string; theme: 'light' | 'dark' } = $props();

	let host = $state<HTMLDivElement | null>(null);
	let root: ShadowRoot | null = null;

	$effect(() => {
		if (!host) return;
		if (!root) {
			root = host.attachShadow({ mode: 'open' });
			// Bound to the root rather than its children, so it outlives every
			// re-render below.
			root.addEventListener('click', onClick);
		}
		mount(root, html);
	});

	// The fragment opens with a <style> holding the whole document stylesheet
	// and, when there is math, a base64 font. That block is the same on every
	// render, so re-parsing it per keystroke is pure cost — the body after it
	// is the only part that actually changes.
	let styleEl: HTMLStyleElement | null = null;
	let mountedStyle = '';

	function mount(target: ShadowRoot, fragment: string) {
		const end = fragment.indexOf('</style>');
		if (!fragment.startsWith('<style>') || end === -1) {
			target.innerHTML = fragment;
			styleEl = null;
			return;
		}
		const css = fragment.slice('<style>'.length, end);
		const body = fragment.slice(end + '</style>'.length);

		if (!styleEl || !styleEl.isConnected) {
			target.innerHTML = '';
			styleEl = document.createElement('style');
			target.append(styleEl);
			mountedStyle = '';
		}
		if (css !== mountedStyle) {
			styleEl.textContent = css;
			mountedStyle = css;
		}
		// Everything after the stylesheet, replaced in one parse.
		while (styleEl.nextSibling) styleEl.nextSibling.remove();
		styleEl.insertAdjacentHTML('afterend', body);
	}

	// The download carries a script for these two behaviours. A fragment does
	// not, and the preview does not want one: making it run would mean lifting
	// a <script> out of the rendered document and executing it at page level,
	// which is a poor thing to hand a renderer whose input is a file someone
	// sent you. Re-implementing the same two behaviours costs less.
	function onClick(e: Event) {
		const target = e.composedPath()[0];
		if (!(target instanceof Element)) return;

		const copy = target.closest('.md2pdf-copy');
		if (copy instanceof HTMLElement) {
			void copyCode(copy);
			return;
		}

		const link = target.closest('a[href^="#"]');
		if (!(link instanceof HTMLAnchorElement) || !root) return;
		// The browser cannot resolve a fragment against ids inside a shadow
		// root, so the jump is ours to make.
		e.preventDefault();
		const toggle = root.getElementById('md2pdf-toc-state');
		if (toggle instanceof HTMLInputElement) toggle.checked = false;
		root
			.getElementById(decodeURIComponent(link.hash.slice(1)))
			?.scrollIntoView({ block: 'start', behavior: 'smooth' });
	}

	async function copyCode(button: HTMLElement) {
		const code = button.parentElement?.querySelector('code');
		if (!code || !navigator.clipboard) return;
		await navigator.clipboard.writeText(code.textContent ?? '');
		const was = button.textContent;
		button.textContent = button.dataset.done ?? was;
		setTimeout(() => {
			button.textContent = was;
		}, 1200);
	}

	// `data-theme` on the host wins over `prefers-color-scheme` inside it.
	$effect(() => {
		host?.setAttribute('data-theme', theme);
	});
</script>

<div class="html-preview" bind:this={host}></div>

<style>
	.html-preview {
		display: block;
		min-height: 100%;
		/* The engine's sheet paints the document; this keeps the pane from
		   flashing white behind it while a dark document mounts. */
		background: var(--md-bg, #fff);
	}
</style>
