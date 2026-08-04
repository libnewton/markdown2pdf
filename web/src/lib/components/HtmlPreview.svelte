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
		root ??= host.attachShadow({ mode: 'open' });
		// Re-parsing the whole fragment costs a millisecond or two and keeps the
		// diffing honest; scroll position is on the pane, not inside the root.
		root.innerHTML = html;
		hoistScript(root);
	});

	// A <script> that arrives through innerHTML is inert, so the document's own
	// behaviour (code copying, outline links) has to be re-executed once. It
	// binds to `document` and resolves its scope from the clicked element, so
	// running it outside the shadow root is exactly right.
	let scriptRun = false;

	function hoistScript(target: ShadowRoot) {
		const inert = [...target.querySelectorAll('script')];
		if (!scriptRun && inert[0]?.textContent) {
			const live = document.createElement('script');
			live.textContent = inert[0].textContent;
			document.head.append(live);
			scriptRun = true;
		}
		inert.forEach((s) => s.remove());
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
