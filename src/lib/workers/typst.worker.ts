/// <reference lib="webworker" />

import { createTypstCompiler, loadFonts, type TypstCompiler } from '@myriaddreamin/typst.ts';
import typstCompilerWasmUrl from '@myriaddreamin/typst-ts-web-compiler/pkg/typst_ts_web_compiler_bg.wasm?url';

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
	format?: 'pdf' | 'vector';
};

type CompileResponse =
	| { type: 'compile-result'; id: string; ok: true; pdf: ArrayBuffer; diagnostics: string[] }
	| { type: 'compile-result'; id: string; ok: true; vector: ArrayBuffer; diagnostics: string[] }
	| { type: 'compile-result'; id: string; ok: false; error: string; diagnostics: string[] };

const ctx: DedicatedWorkerGlobalScope = self as unknown as DedicatedWorkerGlobalScope;

let compilerPromise: Promise<TypstCompiler> | null = null;
let compileQueue: Promise<void> = Promise.resolve();

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

/** Fetch the Twemoji SVGs the document needs into the compiler VFS. */
async function loadTwemoji(compiler: TypstCompiler, markdown: string): Promise<void> {
	const engine = await getEngine();
	const list = new TextDecoder()
		.decode(engine('twemojis', new TextEncoder().encode(markdown)))
		.split('\n')
		.filter((cp) => cp !== '');
	await Promise.all(
		list.map(async (cp) => {
			try {
				const resp = await fetch('/md2pdf/twemoji/' + cp + '.svg');
				if (!resp.ok) return;
				compiler.mapShadow('/md2pdf/twemoji/' + cp + '.svg', new Uint8Array(await resp.arrayBuffer()));
			} catch {
				// A missing glyph just renders as nothing — don't fail the compile.
			}
		})
	);
}

/** The entry document: hand the Markdown to the `md2pdf` package and eval it. */
function buildMain(pageNumbers: boolean): string {
	return `#import "/md2pdf/lib.typ": prepare
#let _d = prepare(read("/doc.md"), page-numbers: ${pageNumbers})
#if not _d.skip { show: _d.template; eval(_d.body, mode: "markup", scope: _d.scope) }
`;
}

async function compileTypst(
	markdown: string,
	images: Record<string, Uint8Array<ArrayBuffer>> = {},
	pageNumbers = true,
	format: 'pdf' | 'vector' = 'pdf'
): Promise<{ result: Uint8Array; diagnostics: string[] }> {
	const compiler = await getCompiler();

	await loadTwemoji(compiler, markdown);

	// The Markdown is a data file read via `read()` — it must live in the
	// shadow VFS (mapShadow), not the source set (addSource).
	compiler.mapShadow('/doc.md', new TextEncoder().encode(markdown));
	compiler.addSource('/main.typ', buildMain(pageNumbers));

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

ctx.onmessage = (event: MessageEvent<CompileRequest>) => {
	const message = event.data;
	if (!message || message.type !== 'compile') return;

	const fmt = message.format || 'pdf';
	compileQueue = compileQueue.then(async () => {
		try {
			const { result, diagnostics } = await compileTypst(
				message.markdown,
				message.images,
				message.pageNumbers,
				fmt
			);
			const copy = new Uint8Array(result.length);
			copy.set(result);
			const response: CompileResponse =
				fmt === 'pdf'
					? { type: 'compile-result', id: message.id, ok: true, pdf: copy.buffer, diagnostics }
					: { type: 'compile-result', id: message.id, ok: true, vector: copy.buffer, diagnostics };
			ctx.postMessage(response, [copy.buffer]);
		} catch (error) {
			ctx.postMessage({
				type: 'compile-result',
				id: message.id,
				ok: false,
				error: error instanceof Error ? error.message : String(error),
				diagnostics: []
			} satisfies CompileResponse);
		}
	});
};
