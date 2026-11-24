import type { SvgDocument } from '#lib/typst/svg-split.ts';
import type { TypstWorkerClient } from '#lib/workers/typstClient.ts';
import { SUPERSEDED, type ImageMap } from '#lib/workers/compileProtocol.ts';

export type LocalImageAsset = {
	bytes: Uint8Array<ArrayBuffer>;
	mimeType: string;
};

type Options = {
	/** The document's own images, by the path the Markdown uses. */
	imageAssets: () => Record<string, LocalImageAsset>;
	/** The text on screen, which a late render compares itself against. */
	markdown: () => string;
	/** Which previews are on screen, so a late remote image can refresh them. */
	pagesVisible: () => boolean;
	webVisible: () => boolean;
	/** The reference view has a source it will not write to. */
	readOnly: () => boolean;
};

/**
 * Both previews: the paged one (a Typst compile, rendered to SVG in the
 * worker) and the Web one (the engine's HTML). Owns their scheduling and the
 * images shipped to the worker; the worker fetches remote images itself.
 */
export class PreviewController {
	status = $state<'idle' | 'compiling' | 'done' | 'error'>('idle');
	errorMessage = $state<string | null>(null);
	previewDoc = $state<SvgDocument | null>(null);
	htmlDoc = $state('');
	// Content the render had to degrade — an image it could not fetch, a diagram
	// it could not draw. The document still renders, which is why these would
	// otherwise pass unnoticed.
	warnings = $state<string[]>([]);

	/** What the paged preview last compiled, so a late remote image can tell
	 * whether the document has moved on since. */
	compiledMarkdown = '';

	/** Set by `connect`, once the worker exists — which is only ever in the browser. */
	client = $state<TypstWorkerClient | null>(null);
	#options: Options;
	#compileSeq = 0;
	#htmlSeq = 0;
	#hasEverCompiled = false;
	// The auto-compile debounce follows the last compile's duration, so a slow
	// document is not re-queued faster than it can be rendered.
	#lastCycleMs = 0;
	#sentImages = new Map<string, Uint8Array<ArrayBuffer>>();

	constructor(options: Options) {
		this.#options = options;
	}

	connect(client: TypstWorkerClient) {
		this.client = client;
		// Remote images are never awaited by a preview: it renders with what
		// the worker has, and renders again once the rest arrive.
		client.onRemotesReady = () => {
			const md = this.#options.markdown();
			if (this.#options.pagesVisible()) void this.compile(md);
			if (this.#options.webVisible()) void this.renderHtml(md);
		};
	}

	/** How long the next auto-compile should wait after a keystroke. */
	get compileDelay(): number {
		return this.#hasEverCompiled ? Math.min(Math.max(450, this.#lastCycleMs), 2500) : 0;
	}

	// All Markdown processing happens inside the Typst compile (the md2pdf
	// engine). The host only resolves images Typst cannot fetch itself.
	async compile(md: string) {
		const client = this.client;
		if (!client) return;
		// Only mark "compiled" once real content exists, so the adaptive delay
		// is not calibrated against an empty first pass.
		if (md.trim() !== '') this.#hasEverCompiled = true;

		const seq = ++this.#compileSeq;
		// Whatever is still queued in the worker is already stale.
		client.cancelPendingPreview();
		this.status = 'compiling';
		this.errorMessage = null;
		const startedAt = performance.now();

		try {
			this.compiledMarkdown = md;
			const result = await client.compilePreview(md, this.newImages());
			if (seq !== this.#compileSeq) return;
			this.previewDoc = result.preview;
			this.status = 'done';
			this.#lastCycleMs = performance.now() - startedAt;
		} catch (error) {
			// A compile the worker dropped in favour of a newer one is not an error.
			if (error instanceof Error && error.message === SUPERSEDED) return;
			if (seq !== this.#compileSeq) return;
			this.status = 'error';
			this.errorMessage = error instanceof Error ? error.message : String(error);
		}
	}

	async renderHtml(md: string) {
		const client = this.client;
		if (!client) return;
		const seq = ++this.#htmlSeq;
		try {
			// `editable` gives the fragment its source lines and live checkboxes;
			// the reference view has a source it will not write to, so it opts out.
			const { html, diagnostics } = await client.renderHtml(
				md,
				this.newImages(),
				false,
				!this.#options.readOnly(),
			);
			// Both guards matter: `seq` catches a render this controller superseded,
			// `md === markdown` catches one that finished against text the document
			// has since moved past — which is how a checkbox click during the
			// debounce window got repainted in its old state.
			if (seq === this.#htmlSeq && md === this.#options.markdown()) {
				this.htmlDoc = html;
				this.warnings = [...new Set(diagnostics)];
			}
		} catch (error) {
			// A render the worker dropped in favour of a newer one is not an error.
			if (error instanceof Error && error.message === SUPERSEDED) return;
			if (seq !== this.#htmlSeq) return;
			this.status = 'error';
			this.errorMessage = error instanceof Error ? error.message : String(error);
		}
	}

	/**
	 * The document's images the worker does not have yet. The worker keeps
	 * every image it has been handed, so a recompile only ships new ones
	 * instead of copying megabytes across the worker boundary. All of them go,
	 * not just the ones the body references: a cover or header image is named
	 * in the frontmatter, and the engine is what knows where to look.
	 */
	newImages(): ImageMap {
		const fresh: ImageMap = {};
		for (const [path, { bytes }] of Object.entries(this.#options.imageAssets())) {
			if (this.#sentImages.get(path) === bytes) continue;
			this.#sentImages.set(path, bytes);
			fresh[path] = bytes;
		}
		return fresh;
	}
}
