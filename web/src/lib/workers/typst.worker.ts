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

// Markdown processing lives entirely in the `md2pdf` Typst package — the
// Rust/WASM engine runs inside the compile. This worker only feeds raw
// Markdown + images to Typst, and pre-fetches the Twemoji SVGs the engine
// asks for (Typst's sandbox cannot fetch them itself).

type CompileRequest = {
	type: 'compile';
	id: string;
	markdown: string;
	images?: Record<string, Uint8Array<ArrayBuffer>>;
	pageNumbers?: boolean;
	format?: 'pdf' | 'preview';
};

type CompileResponse =
	| { type: 'compile-result'; id: string; ok: true; pdf: ArrayBuffer; diagnostics: string[] }
	| { type: 'compile-result'; id: string; ok: true; preview: SvgDocument; diagnostics: string[] }
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

let enginePromise: Promise<(fn: string, arg: Uint8Array) => Uint8Array> | null = null;

function getEngine(): Promise<(fn: string, arg: Uint8Array) => Uint8Array> {
	if (enginePromise) return enginePromise;
	enginePromise = (async () => {
		const bytes = await fetch('/md2pdf/engine.wasm').then((r) => r.arrayBuffer());
		let pendingArgs: Uint8Array[] = [];
		let result: Uint8Array | null = null;
		let instance: WebAssembly.Instance;
		// The wasm-minimal-protocol host side.
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
		return (fn: string, arg: Uint8Array): Uint8Array => {
			pendingArgs = [arg];
			result = null;
			const ret = (instance.exports[fn] as (n: number) => number)(arg.length);
			if (ret !== 0) {
				throw new Error('engine error: ' + new TextDecoder().decode(result ?? new Uint8Array()));
			}
			return result ?? new Uint8Array();
		};
	})();
	return enginePromise;
}

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

// Twemoji codepoints already fetched + mapped into the compiler VFS. Kept
// across recompiles so the live-preview loop doesn't re-fetch the same SVGs
// on every keystroke.
const mappedTwemoji = new Set<string>();

/** Fetch the Twemoji SVGs the document needs into the compiler VFS. */
async function loadTwemoji(compiler: TypstCompiler, markdown: string): Promise<void> {
	const engine = await getEngine();
	const list = new TextDecoder()
		.decode(engine('twemojis', new TextEncoder().encode(markdown)))
		.split('\n')
		.filter((cp) => cp !== '' && !mappedTwemoji.has(cp));
	await Promise.all(
		list.map(async (cp) => {
			try {
				const resp = await fetch('/md2pdf/twemoji/' + cp + '.svg');
				if (!resp.ok) return;
				compiler.mapShadow('/md2pdf/twemoji/' + cp + '.svg', new Uint8Array(await resp.arrayBuffer()));
				mappedTwemoji.add(cp);
			} catch {
				// A missing glyph just renders as nothing — don't fail the compile.
			}
		})
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

ctx.onmessage = (event: MessageEvent<CompileRequest | CancelRequest>) => {
	const message = event.data;
	if (!message) return;

	if (message.type === 'cancel') {
		const i = queue.findIndex((q) => q.id === message.id);
		if (i !== -1) {
			queue.splice(i, 1);
			reply(message.id, SUPERSEDED);
		}
		return;
	}

	if (message.type !== 'compile') return;

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
