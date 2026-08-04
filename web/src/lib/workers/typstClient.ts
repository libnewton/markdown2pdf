import type { SvgDocument } from '$lib/typst/svg-split';

type CompileRequest = {
	type: 'compile';
	id: string;
	markdown: string;
	images?: Record<string, Uint8Array<ArrayBuffer>>;
	format?: 'pdf' | 'preview';
};

type HtmlRequest = {
	type: 'html';
	id: string;
	markdown: string;
	images?: Record<string, Uint8Array<ArrayBuffer>>;
	standalone?: boolean;
};

type CompileResponse =
	| {
			type: 'compile-result';
			id: string;
			ok: true;
			pdf: ArrayBuffer;
			diagnostics: string[];
	  }
	| {
			type: 'compile-result';
			id: string;
			ok: true;
			preview: SvgDocument;
			diagnostics: string[];
	  }
	| {
			type: 'compile-result';
			id: string;
			ok: true;
			html: string;
			diagnostics: string[];
	  }
	| {
			type: 'compile-result';
			id: string;
			ok: false;
			error: string;
			diagnostics: string[];
	  };

type CompileResult = {
	pdf?: Uint8Array<ArrayBuffer>;
	preview?: SvgDocument;
	html?: string;
	diagnostics: string[];
};

type Pending = {
	resolve: (value: CompileResult) => void;
	reject: (reason: unknown) => void;
};

export class TypstWorkerClient {
	#worker: Worker;
	#pending = new Map<string, Pending>();
	/** Id of the preview compile that has not returned yet, if any. */
	#pendingPreviewId: string | null = null;

	constructor() {
		this.#worker = new Worker(new URL('./typst.worker.ts', import.meta.url), { type: 'module' });
		this.#worker.addEventListener('message', (event: MessageEvent<CompileResponse>) => {
			const message = event.data;
			if (!message || message.type !== 'compile-result') return;

			const pending = this.#pending.get(message.id);
			if (!pending) return;
			this.#pending.delete(message.id);
			if (this.#pendingPreviewId === message.id) this.#pendingPreviewId = null;

			if (!message.ok) {
				pending.reject(new Error(message.error));
				return;
			}

			const result: CompileResult = { diagnostics: message.diagnostics };
			if ('pdf' in message) result.pdf = new Uint8Array(message.pdf);
			if ('preview' in message) result.preview = message.preview;
			if ('html' in message) result.html = message.html;
			pending.resolve(result);
		});
	}

	dispose(): void {
		this.#worker.terminate();
		for (const pending of this.#pending.values()) {
			pending.reject(new Error('Worker terminated'));
		}
		this.#pending.clear();
		this.#pendingPreviewId = null;
	}

	/**
	 * Render the document to HTML. This never enters the compile queue — the
	 * engine produces HTML on its own, so it stays instant even while a PDF
	 * export is running. `standalone` wraps the fragment in a full document
	 * for download; the preview pane mounts the fragment in a shadow root.
	 */
	renderHtml(
		markdown: string,
		images: Record<string, Uint8Array<ArrayBuffer>> = {},
		standalone = false
	): Promise<string> {
		const id = this.#nextId();
		const request: HtmlRequest = { type: 'html', id, markdown, images, standalone };
		return new Promise<CompileResult>((resolve, reject) => {
			this.#pending.set(id, { resolve, reject });
			this.#worker.postMessage(request);
		}).then((r) => r.html ?? '');
	}

	/**
	 * Ask the worker to drop a queued preview compile. Only requests that have
	 * not started are dropped; one already running finishes (its result is
	 * discarded by the caller's sequence check).
	 */
	cancelPendingPreview(): void {
		if (!this.#pendingPreviewId) return;
		this.#worker.postMessage({ type: 'cancel', id: this.#pendingPreviewId });
		this.#pendingPreviewId = null;
	}

	compilePdf(
		markdown: string,
		images: Record<string, Uint8Array<ArrayBuffer>> = {}
	): Promise<{ pdf: Uint8Array<ArrayBuffer>; diagnostics: string[] }> {
		return this.#compile(markdown, images, 'pdf').then((r) => ({
			pdf: r.pdf!,
			diagnostics: r.diagnostics
		}));
	}

	/** Compile and render the preview: page markup, ready to display. */
	compilePreview(
		markdown: string,
		images: Record<string, Uint8Array<ArrayBuffer>> = {}
	): Promise<{ preview: SvgDocument; diagnostics: string[] }> {
		return this.#compile(markdown, images, 'preview').then((r) => ({
			preview: r.preview!,
			diagnostics: r.diagnostics
		}));
	}

	#nextId(): string {
		return typeof crypto !== 'undefined' && 'randomUUID' in crypto
			? crypto.randomUUID()
			: String(Date.now()) + Math.random().toString(36).slice(2);
	}

	#compile(
		markdown: string,
		images: Record<string, Uint8Array<ArrayBuffer>>,
		format: 'pdf' | 'preview'
	): Promise<CompileResult> {
		const id = this.#nextId();
		const request: CompileRequest = { type: 'compile', id, markdown, images, format };
		if (format === 'preview') this.#pendingPreviewId = id;

		return new Promise((resolve, reject) => {
			this.#pending.set(id, { resolve, reject });
			this.#worker.postMessage(request);
		});
	}
}

let sharedTypstWorkerClient: TypstWorkerClient | null = null;

export function getSharedTypstWorkerClient(): TypstWorkerClient {
	if (!sharedTypstWorkerClient) {
		sharedTypstWorkerClient = new TypstWorkerClient();
	}

	return sharedTypstWorkerClient;
}
