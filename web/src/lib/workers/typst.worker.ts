/// <reference lib="webworker" />

import {
	createTypstCompiler,
	createTypstRenderer,
	loadFonts,
	type TypstCompiler,
	type TypstRenderer
} from '@myriaddreamin/typst.ts';
import typstCompilerWasmUrl from '@myriaddreamin/typst-ts-web-compiler/pkg/typst_ts_web_compiler_bg.wasm?url';
import typstRendererWasmUrl from '@myriaddreamin/typst-ts-renderer/pkg/typst_ts_renderer_bg.wasm?url';
import { SUPERSEDED } from './compileProtocol';
import { splitSvgDocument, type SvgDocument } from '$lib/typst/svg-split';
import { buildAssetBundle, parseKeyedSources, type Asset } from './assetBundle';

// Markdown processing lives entirely in the `md2pdf` Typst package — the
// Rust/WASM engine runs inside the compile. This worker only feeds raw
// Markdown + images to Typst, and pre-fetches the Twemoji SVGs the engine
// asks for (Typst's sandbox cannot fetch them itself).
//
// The HTML target is different: the engine renders it on its own, so that path
// calls engine.wasm directly and never touches the Typst compiler. It is fast
// enough (single-digit milliseconds) to run on every keystroke, which is why it
// also bypasses the compile queue.

type CompileRequest = {
	type: 'compile';
	id: string;
	markdown: string;
	images?: Record<string, Uint8Array<ArrayBuffer>>;
	pageNumbers?: boolean;
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
	| { type: 'compile-result'; id: string; ok: true; pdf: ArrayBuffer; diagnostics: string[] }
	| { type: 'compile-result'; id: string; ok: true; preview: SvgDocument; diagnostics: string[] }
	| { type: 'compile-result'; id: string; ok: true; html: string; diagnostics: string[] }
	| { type: 'compile-result'; id: string; ok: false; error: string; diagnostics: string[] };

type CancelRequest = { type: 'cancel'; id: string };

const ctx: DedicatedWorkerGlobalScope = self as unknown as DedicatedWorkerGlobalScope;

let compilerPromise: Promise<TypstCompiler> | null = null;

// Fonts served same-origin from /fonts/* (bundled at build time). No CDNs.
const CORE_FONTS: string[] = [
	'/fonts/IBMPlexSans-Regular.ttf',
	'/fonts/IBMPlexSans-Bold.ttf',
	'/fonts/NewCMMath-Regular.otf',
	'/fonts/NewCMMath-Book.otf',
	'/fonts/DejaVuSansMono.ttf',
	'/fonts/DejaVuSansMono-Bold.ttf',
	'/fonts/DejaVuSansMono-Oblique.ttf',
	'/fonts/DejaVuSansMono-BoldOblique.ttf'
];

// --------------------------------------------------------------------------
// Direct access to the engine WASM (the same module Typst loads via plugin()).
// Used to ask the engine which Twemoji SVGs the document needs, so the worker
// can pre-fetch them into the VFS before the compile.
// --------------------------------------------------------------------------

/** Calls one exported function of a `wasm-minimal-protocol` Typst plugin. */
type Plugin = (fn: string, ...args: Uint8Array[]) => Uint8Array;

const plugins = new Map<string, Promise<Plugin>>();

/** Instantiate a Typst WASM plugin outside Typst, implementing its host ABI. */
function loadPlugin(url: string): Promise<Plugin> {
	let cached = plugins.get(url);
	if (cached) return cached;
	cached = (async () => {
		const bytes = await fetch(url).then((r) => r.arrayBuffer());
		let pendingArgs: Uint8Array[] = [];
		let result: Uint8Array | null = null;
		let instance: WebAssembly.Instance;
		const env = {
			wasm_minimal_protocol_write_args_to_buffer(ptr: number) {
				const mem = new Uint8Array((instance.exports.memory as WebAssembly.Memory).buffer);
				let off = ptr;
				for (const a of pendingArgs) {
					mem.set(a, off);
					off += a.length;
				}
			},
			wasm_minimal_protocol_send_result_to_host(ptr: number, len: number) {
				result = new Uint8Array(
					(instance.exports.memory as WebAssembly.Memory).buffer,
					ptr,
					len
				).slice();
			}
		};
		instance = (await WebAssembly.instantiate(bytes, { typst_env: env })).instance;
		return (fn: string, ...args: Uint8Array[]): Uint8Array => {
			pendingArgs = args;
			result = null;
			const exported = instance.exports[fn] as (...lengths: number[]) => number;
			const ret = exported(...args.map((a) => a.length));
			if (ret !== 0) {
				throw new Error('engine error: ' + new TextDecoder().decode(result ?? new Uint8Array()));
			}
			return result ?? new Uint8Array();
		};
	})();
	plugins.set(url, cached);
	return cached;
}

const getEngine = () => loadPlugin('/md2pdf/engine.wasm');
/** Mermaid is 3.9 MB, so it only loads once a document actually has a diagram. */
const getMermaid = () => loadPlugin('/md2pdf/vendor/mmdr/typst_mmdr.wasm');

const encode = (s: string) => new TextEncoder().encode(s);
const decode = (b: Uint8Array) => new TextDecoder().decode(b);
const lines = (b: Uint8Array) => decode(b).split('\n').filter((l) => l !== '');

// --------------------------------------------------------------------------
// Compiler setup
// --------------------------------------------------------------------------

/**
 * Register the bundled `md2pdf` Typst package (engine.wasm + lib.typ + styles
 * + vendored mitex/mmdr) into the compiler VFS under `/md2pdf/`.
 */
async function registerPackage(compiler: TypstCompiler): Promise<void> {
	const manifest: string[] = await fetch('/md2pdf/manifest.json').then((r) => r.json());
	await Promise.all(
		manifest.map(async (rel) => {
			const resp = await fetch('/md2pdf/' + rel);
			const vpath = '/md2pdf/' + rel;
			if (rel.endsWith('.wasm')) {
				compiler.mapShadow(vpath, new Uint8Array(await resp.arrayBuffer()));
			} else {
				compiler.addSource(vpath, await resp.text());
			}
		})
	);
}

function getCompiler(): Promise<TypstCompiler> {
	if (!compilerPromise) {
		compilerPromise = (async () => {
			const compiler = createTypstCompiler();
			await compiler.init({
				getModule: () => typstCompilerWasmUrl,
				beforeBuild: [loadFonts(CORE_FONTS, { assets: false })]
			});
			await registerPackage(compiler);
			return compiler;
		})();
	}
	return compilerPromise;
}

// The preview renderer runs here too: turning the artifact into SVG is a long
// synchronous WASM call that used to block typing on the main thread.
let rendererPromise: Promise<TypstRenderer> | null = null;

function getRenderer(): Promise<TypstRenderer> {
	if (!rendererPromise) {
		rendererPromise = (async () => {
			const renderer = createTypstRenderer();
			await renderer.init({ getModule: () => typstRendererWasmUrl });
			return renderer;
		})();
	}
	return rendererPromise;
}

async function renderPreview(artifact: Uint8Array): Promise<SvgDocument> {
	const renderer = await getRenderer();
	let svg = '';
	await renderer.runWithSession({ format: 'vector', artifactContent: artifact }, async (session) => {
		svg = await session.renderSvg({
			data_selection: { body: true, defs: true, css: true, js: false }
		});
	});
	return splitSvgDocument(svg);
}

// Twemoji SVGs already fetched, by codepoint. Kept across recompiles so the
// live-preview loop doesn't re-fetch the same glyphs on every keystroke; the
// HTML target embeds the same bytes as data: URIs.
const twemojiCache = new Map<string, Uint8Array>();

async function fetchTwemoji(codepoints: string[]): Promise<void> {
	await Promise.all(
		codepoints
			.filter((cp) => !twemojiCache.has(cp))
			.map(async (cp) => {
				try {
					const resp = await fetch('/md2pdf/twemoji/' + cp + '.svg');
					if (!resp.ok) return;
					twemojiCache.set(cp, new Uint8Array(await resp.arrayBuffer()));
				} catch {
					// A missing glyph just renders as nothing — don't fail the render.
				}
			})
	);
}

// Codepoints already written into the compiler VFS (a superset check would
// re-map identical bytes and cost Typst its incremental caches).
const mappedTwemoji = new Set<string>();

/** Fetch the Twemoji SVGs the document needs into the compiler VFS. */
async function loadTwemoji(compiler: TypstCompiler, markdown: string): Promise<void> {
	const engine = await getEngine();
	const list = lines(engine('twemojis', encode(markdown)));
	await fetchTwemoji(list);
	for (const cp of list) {
		const bytes = twemojiCache.get(cp);
		if (!bytes || mappedTwemoji.has(cp)) continue;
		compiler.mapShadow('/md2pdf/twemoji/' + cp + '.svg', bytes);
		mappedTwemoji.add(cp);
	}
}

// --------------------------------------------------------------------------
// HTML target — engine only, no Typst compile
// --------------------------------------------------------------------------

// Image bytes seen so far. Requests carry only what changed, mirroring how the
// compiler VFS accumulates, so the store has to remember the rest.
const imageStore = new Map<string, Uint8Array>();
// Rendered Mermaid diagrams, keyed the way the engine asks for them.
const mermaidCache = new Map<string, Uint8Array>();

async function renderMermaid(markdown: string): Promise<Asset[]> {
	const engine = await getEngine();
	const wanted = parseKeyedSources(decode(engine('html_mermaid', encode(markdown))));
	const missing = wanted.filter((d) => !mermaidCache.has(d.key));
	if (missing.length > 0) {
		const mermaid = await getMermaid();
		for (const { key, source } of missing) {
			try {
				mermaidCache.set(key, mermaid('render', encode(source), encode('modern'), encode(''), encode('')));
			} catch {
				// A diagram that won't render falls back to its source in the output.
			}
		}
	}
	return wanted.flatMap((d) => {
		const svg = mermaidCache.get(d.key);
		return svg ? [[d.key, svg] as Asset] : [];
	});
}

/**
 * Render the document to HTML. Everything the engine cannot reach itself —
 * image bytes, Twemoji art, Mermaid diagrams — is resolved here and handed
 * over as one blob plus a `key<TAB>byte-length` manifest, exactly as the
 * Typst package does for the CLI.
 */
async function renderHtml(markdown: string, standalone: boolean): Promise<string> {
	const engine = await getEngine();
	const md = encode(markdown);

	const assets: Asset[] = [];
	for (const path of lines(engine('html_images', md))) {
		const bytes = imageStore.get(path);
		if (bytes) assets.push([path, bytes]);
	}
	for (const line of lines(engine('remotes', md))) {
		const alias = line.slice(line.indexOf('\t') + 1);
		const bytes = imageStore.get(alias);
		if (bytes) assets.push([alias, bytes]);
	}
	const codepoints = lines(engine('twemojis', md));
	await fetchTwemoji(codepoints);
	for (const cp of codepoints) {
		const bytes = twemojiCache.get(cp);
		if (bytes) assets.push(['twemoji/' + cp + '.svg', bytes]);
	}
	assets.push(...(await renderMermaid(markdown)));

	const { manifest, blob } = buildAssetBundle(assets);
	return decode(
		engine(
			'render_html',
			md,
			encode(`standalone=${standalone ? 1 : 0}`),
			encode(manifest),
			blob
		)
	);
}

/**
 * The entry document: hand the Markdown to the `md2pdf` package and eval it.
 * `page-numbers` is only a default — a frontmatter `pageNumbers:` wins. `asset`
 * must be defined here, not in the package: an `image()` call written inside a
 * Typst package resolves against the package root, not the document root.
 */
function buildMain(pageNumbersDefault: boolean): string {
	return `#import "/md2pdf/lib.typ": prepare
#let _d = prepare(read("/doc.md"), page-numbers: ${pageNumbersDefault}, asset: (p, ..a) => image(p, ..a))
#if not _d.skip { show: _d.template; eval(_d.body, mode: "markup", scope: _d.scope) }
`;
}

// What the VFS already holds, so a live-preview recompile only writes what
// actually changed. Re-mapping identical bytes would throw away Typst's
// incremental caches for those files (images get re-decoded, the emoji scan
// re-runs) on every keystroke.
let mappedMarkdown: string | null = null;
let mappedMain: string | null = null;

async function compileTypst(
	markdown: string,
	images: Record<string, Uint8Array<ArrayBuffer>> = {},
	pageNumbers = true,
	format: 'pdf' | 'preview' = 'pdf'
): Promise<{ result: Uint8Array; diagnostics: string[] }> {
	const compiler = await getCompiler();

	if (markdown !== mappedMarkdown) {
		await loadTwemoji(compiler, markdown);
		// The Markdown is a data file read via `read()` — it must live in the
		// shadow VFS (mapShadow), not the source set (addSource).
		compiler.mapShadow('/doc.md', new TextEncoder().encode(markdown));
		mappedMarkdown = markdown;
	}

	const main = buildMain(pageNumbers);
	if (main !== mappedMain) {
		compiler.addSource('/main.typ', main);
		mappedMain = main;
	}

	for (const [path, data] of Object.entries(images)) {
		compiler.mapShadow('/' + path, data);
	}

	const compileResult = await compiler.compile({
		mainFilePath: '/main.typ',
		format: format === 'pdf' ? 1 : 0,
		diagnostics: 'unix'
	});

	const diagnostics = (compileResult.diagnostics ?? []).map(String);
	if (!compileResult.result) {
		throw new Error(diagnostics.join('\n') || 'Typst compilation failed (no diagnostics)');
	}
	return { result: compileResult.result, diagnostics };
}

// While one compile runs, more keystrokes arrive; only the newest of those is
// worth running. Export requests are user-initiated and are never dropped.
let running = false;
const queue: CompileRequest[] = [];

function reply(id: string, error: string) {
	ctx.postMessage({
		type: 'compile-result',
		id,
		ok: false,
		error,
		diagnostics: []
	} satisfies CompileResponse);
}

async function runOne(message: CompileRequest): Promise<void> {
	const fmt = message.format || 'pdf';
	try {
		const { result, diagnostics } = await compileTypst(
			message.markdown,
			message.images,
			message.pageNumbers,
			fmt
		);
		if (fmt === 'pdf') {
			const copy = new Uint8Array(result.length);
			copy.set(result);
			ctx.postMessage(
				{ type: 'compile-result', id: message.id, ok: true, pdf: copy.buffer, diagnostics },
				[copy.buffer]
			);
			return;
		}
		const preview = await renderPreview(result);
		ctx.postMessage({ type: 'compile-result', id: message.id, ok: true, preview, diagnostics });
	} catch (error) {
		reply(message.id, error instanceof Error ? error.message : String(error));
	}
}

async function drain(): Promise<void> {
	if (running) return;
	running = true;
	try {
		while (queue.length > 0) {
			await runOne(queue.shift()!);
		}
	} finally {
		running = false;
	}
}

function rememberImages(images?: Record<string, Uint8Array<ArrayBuffer>>): void {
	for (const [path, data] of Object.entries(images ?? {})) {
		imageStore.set(path, data);
	}
}

ctx.onmessage = (event: MessageEvent<CompileRequest | HtmlRequest | CancelRequest>) => {
	const message = event.data;
	if (!message) return;

	// HTML needs no Typst compile, so it skips the queue entirely and stays
	// responsive while a PDF or preview compile is still running.
	if (message.type === 'html') {
		rememberImages(message.images);
		renderHtml(message.markdown, message.standalone ?? false)
			.then((html) =>
				ctx.postMessage({
					type: 'compile-result',
					id: message.id,
					ok: true,
					html,
					diagnostics: []
				} satisfies CompileResponse)
			)
			.catch((error) => reply(message.id, error instanceof Error ? error.message : String(error)));
		return;
	}

	if (message.type === 'cancel') {
		const i = queue.findIndex((q) => q.id === message.id);
		if (i !== -1) {
			queue.splice(i, 1);
			reply(message.id, SUPERSEDED);
		}
		return;
	}

	if (message.type !== 'compile') return;
	rememberImages(message.images);

	if (message.format === 'preview') {
		const i = queue.findIndex((q) => q.format === 'preview');
		if (i !== -1) {
			reply(queue[i].id, SUPERSEDED);
			queue.splice(i, 1);
		}
	}
	queue.push(message);
	void drain();
};
