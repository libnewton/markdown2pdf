<script lang="ts">
	// Pageless HTML view of the document.
	//
	// The markup and its stylesheet come from the engine — byte for byte the
	// same thing the download and the CLI produce. It is mounted in a shadow
	// root so the document's CSS and the app's CSS can never reach each other.

	let {
		html = '',
		theme = 'auto'
	}: { html?: string; theme?: 'auto' | 'light' | 'dark' } = $props();

	let host = $state<HTMLDivElement | null>(null);
	let root: ShadowRoot | null = null;

	$effect(() => {
		if (!host) return;
		root ??= host.attachShadow({ mode: 'open' });
		// Re-parsing the whole fragment costs a millisecond or two and keeps the
		// diffing honest; scroll position is on the pane, not inside the root.
		root.innerHTML = html;
	});

	// `data-theme` on the host wins over `prefers-color-scheme` inside it.
	$effect(() => {
		if (!host) return;
		if (theme === 'auto') host.removeAttribute('data-theme');
		else host.setAttribute('data-theme', theme);
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
