/// <reference lib="webworker" />

import {
	CompileError,
	createTypstCompiler,
	supportsJspiBackend,
	supportsWorkerBackend,
	type Diagnostic,
	type TypstCompiler
} from 'typst-wasm';
import { createWebWorker } from 'typst-wasm/worker/browser';
import workerUrl from 'typst-wasm/worker/web-worker?worker&url';
import coreUrl from 'typst-wasm/engine/engine.core.wasm?url';
import core2Url from 'typst-wasm/engine/engine.core2.wasm?url';
import core3Url from 'typst-wasm/engine/engine.core3.wasm?url';
import { SUPERSEDED } from './compileProtocol';
import { svgPagesDocument, type SvgDocument } from '$lib/typst/svg-split';

type CompileFormat = 'pdf' | 'preview' | 'html';

type CompileRequest = {
	type: 'compile';
	id: string;
	markdown: string;
	images?: Record<string, Uint8Array<ArrayBuffer>>;
	pageNumbers?: boolean;
	format?: CompileFormat;
};

type CompileResponse =
	| { type: 'compile-result'; id: string; ok: true; pdf: ArrayBuffer; diagnostics: string[] }
	| { type: 'compile-result'; id: string; ok: true; preview: SvgDocument; diagnostics: string[] }
	| { type: 'compile-result'; id: string; ok: true; html: string; diagnostics: string[] }
	| { type: 'compile-result'; id: string; ok: false; error: string; diagnostics: string[] };

type CancelRequest = { type: 'cancel'; id: string };

const ctx: DedicatedWorkerGlobalScope = self as unknown as DedicatedWorkerGlobalScope;

const CORE_FONTS = [
	'/fonts/IBMPlexSans-Regular.ttf',
	'/fonts/IBMPlexSans-Bold.ttf',
	'/fonts/NewCMMath-Regular.otf',
	'/fonts/NewCMMath-Book.otf',
	'/fonts/DejaVuSansMono.ttf',
	'/fonts/DejaVuSansMono-Bold.ttf',
	'/fonts/DejaVuSansMono-Oblique.ttf',
	'/fonts/DejaVuSansMono-BoldOblique.ttf'
];

let enginePromise: Promise<(fn: string, arg: Uint8Array) => Uint8Array> | null = null;

function getEngine(): Promise<(fn: string, arg: Uint8Array) => Uint8Array> {
	if (enginePromise) return enginePromise;
	enginePromise = (async () => {
		const bytes = await fetch('/md2pdf/engine.wasm').then((response) => response.arrayBuffer());
		let pendingArgs: Uint8Array[] = [];
		let result: Uint8Array | null = null;
		let instance: WebAssembly.Instance;
		const env = {
			wasm_minimal_protocol_write_args_to_buffer(ptr: number) {
				const memory = new Uint8Array((instance.exports.memory as WebAssembly.Memory).buffer);
				let offset = ptr;
				for (const arg of pendingArgs) {
					memory.set(arg, offset);
					offset += arg.length;
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
			const code = (instance.exports[fn] as (length: number) => number)(arg.length);
			if (code !== 0) {
				throw new Error('engine error: ' + new TextDecoder().decode(result ?? new Uint8Array()));
			}
			return result ?? new Uint8Array();
		};
	})();
	return enginePromise;
}

async function registerPackage(compiler: TypstCompiler): Promise<void> {
	const manifest: string[] = await fetch('/md2pdf/manifest.json').then((response) => response.json());
	await Promise.all(
		manifest.map(async (relativePath) => {
			const response = await fetch('/md2pdf/' + relativePath);
			const path = 'md2pdf/' + relativePath;
			if (relativePath.endsWith('.typ')) {
				await compiler.addSource(path, await response.text());
			} else {
				await compiler.addFile(path, new Uint8Array(await response.arrayBuffer()));
			}
		})
	);
}

let compilerPromise: Promise<TypstCompiler> | null = null;

function getCompiler(): Promise<TypstCompiler> {
	if (compilerPromise) return compilerPromise;
	compilerPromise = (async () => {
		if (!supportsWorkerBackend() && !supportsJspiBackend()) {
			throw new Error(
				'This browser has no compatible Typst backend. Reload once to enable cross-origin isolation, or use a browser with WebAssembly JSPI support.'
			);
		}
		const compiler = await createTypstCompiler({
			backend: 'auto',
			worker: () => createWebWorker(workerUrl),
			coreModules: {
				'engine.core.wasm': WebAssembly.compileStreaming(fetch(coreUrl)),
				'engine.core2.wasm': WebAssembly.compileStreaming(fetch(core2Url)),
				'engine.core3.wasm': WebAssembly.compileStreaming(fetch(core3Url))
			},
			packageCache: false
		});
		await compiler.addFonts(
			...CORE_FONTS.map(async (path) => new Uint8Array(await (await fetch(path)).arrayBuffer()))
		);
		await registerPackage(compiler);
		return compiler;
	})();
	return compilerPromise;
}

const mappedTwemoji = new Set<string>();

async function loadTwemoji(compiler: TypstCompiler, markdown: string): Promise<void> {
	const engine = await getEngine();
	const codepoints = new TextDecoder()
		.decode(engine('twemojis', new TextEncoder().encode(markdown)))
		.split('\n')
		.filter((codepoint) => codepoint !== '' && !mappedTwemoji.has(codepoint));
	await Promise.all(
		codepoints.map(async (codepoint) => {
			try {
				const response = await fetch('/md2pdf/twemoji/' + codepoint + '.svg');
				if (!response.ok) return;
				await compiler.addFile(
					'md2pdf/twemoji/' + codepoint + '.svg',
					new Uint8Array(await response.arrayBuffer())
				);
				mappedTwemoji.add(codepoint);
			} catch {
				// A missing optional glyph must not block the document.
			}
		})
	);
}

function buildMain(pageNumbersDefault: boolean): string {
	return `#import "/md2pdf/lib.typ": prepare
#let _d = prepare(read("/doc.md"), page-numbers: ${pageNumbersDefault}, asset: (p, ..a) => image(p, ..a))
#if not _d.skip { show: _d.template; eval(_d.body, mode: "markup", scope: _d.scope) }
`;
}

let mappedMarkdown: string | null = null;
let mappedMain: string | null = null;

function diagnosticsText(diagnostics: Diagnostic[]): string[] {
	return diagnostics.map((diagnostic) => diagnostic.formatted || diagnostic.message);
}

async function compileTypst(
	markdown: string,
	images: Record<string, Uint8Array<ArrayBuffer>> = {},
	pageNumbers = true,
	format: CompileFormat = 'pdf'
): Promise<
	| { format: 'pdf'; output: Uint8Array; diagnostics: string[] }
	| { format: 'preview'; output: SvgDocument; diagnostics: string[] }
	| { format: 'html'; output: string; diagnostics: string[] }
> {
	const compiler = await getCompiler();
	if (markdown !== mappedMarkdown) {
		await loadTwemoji(compiler, markdown);
		await compiler.addFile('doc.md', new TextEncoder().encode(markdown));
		mappedMarkdown = markdown;
	}

	const main = buildMain(pageNumbers);
	if (main !== mappedMain) {
		await compiler.addSource('main.typ', main);
		mappedMain = main;
	}

	await Promise.all(
		Object.entries(images).map(([path, data]) => compiler.addFile(path, data))
	);

	if (format === 'pdf') {
		const result = await compiler.compile({ main: 'main.typ', format: 'pdf' });
		return { format, output: result.output, diagnostics: diagnosticsText(result.diagnostics) };
	}
	if (format === 'html') {
		const result = await compiler.compile({ main: 'main.typ', format: 'html' });
		return { format, output: result.output, diagnostics: diagnosticsText(result.diagnostics) };
	}
	const result = await compiler.compile({ main: 'main.typ', format: 'svg' });
	return {
		format,
		output: svgPagesDocument(result.pages.map((page) => page.output)),
		diagnostics: diagnosticsText(result.diagnostics)
	};
}

let running = false;
const queue: CompileRequest[] = [];

function reply(id: string, error: string, diagnostics: string[] = []) {
	ctx.postMessage({
		type: 'compile-result',
		id,
		ok: false,
		error,
		diagnostics
	} satisfies CompileResponse);
}

async function runOne(message: CompileRequest): Promise<void> {
	const format = message.format || 'pdf';
	try {
		const result = await compileTypst(
			message.markdown,
			message.images,
			message.pageNumbers,
			format
		);
		if (result.format === 'pdf') {
			const copy = result.output.slice();
			ctx.postMessage(
				{ type: 'compile-result', id: message.id, ok: true, pdf: copy.buffer, diagnostics: result.diagnostics },
				[copy.buffer]
			);
		} else if (result.format === 'html') {
			ctx.postMessage({
				type: 'compile-result',
				id: message.id,
				ok: true,
				html: result.output,
				diagnostics: result.diagnostics
			} satisfies CompileResponse);
		} else {
			ctx.postMessage({
				type: 'compile-result',
				id: message.id,
				ok: true,
				preview: result.output,
				diagnostics: result.diagnostics
			} satisfies CompileResponse);
		}
	} catch (error) {
		const diagnostics = error instanceof CompileError ? diagnosticsText(error.diagnostics) : [];
		reply(message.id, error instanceof Error ? error.message : String(error), diagnostics);
	}
}

async function drain(): Promise<void> {
	if (running) return;
	running = true;
	try {
		while (queue.length > 0) await runOne(queue.shift()!);
	} finally {
		running = false;
	}
}

ctx.onmessage = (event: MessageEvent<CompileRequest | CancelRequest>) => {
	const message = event.data;
	if (!message) return;
	if (message.type === 'cancel') {
		const index = queue.findIndex((request) => request.id === message.id);
		if (index !== -1) {
			queue.splice(index, 1);
			reply(message.id, SUPERSEDED);
		}
		return;
	}
	if (message.type !== 'compile') return;
	if (message.format !== 'pdf') {
		const index = queue.findIndex((request) => request.format !== 'pdf');
		if (index !== -1) {
			reply(queue[index].id, SUPERSEDED);
			queue.splice(index, 1);
		}
	}
	queue.push(message);
	void drain();
};
