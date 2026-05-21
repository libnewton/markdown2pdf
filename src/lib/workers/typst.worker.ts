/// <reference lib="webworker" />

import { createTypstCompiler, loadFonts, type TypstCompiler } from '@myriaddreamin/typst.ts';
import typstCompilerWasmUrl from '@myriaddreamin/typst-ts-web-compiler/pkg/typst_ts_web_compiler_bg.wasm?url';
import modernTechTyp from '../typst/styles/modern-tech.typ?raw';
import redbookKnowledgeTyp from '../typst/styles/redbook-knowledge.typ?raw';
import redbookDarkTyp from '../typst/styles/redbook-dark.typ?raw';
import redbookMinimalistTyp from '../typst/styles/redbook-minimalist.typ?raw';
import redbookForestTyp from '../typst/styles/redbook-forest.typ?raw';
import redbookBlueprintTyp from '../typst/styles/redbook-blueprint.typ?raw';
import redbookCleanTyp from '../typst/styles/redbook-clean.typ?raw';
import redbookModernTyp from '../typst/styles/redbook-modern.typ?raw';
import redbookTypographyTyp from '../typst/styles/redbook-typography.typ?raw';
import slidesModernTyp from '../typst/styles/slides-modern.typ?raw';
import slidesDarkTyp from '../typst/styles/slides-dark.typ?raw';
import slidesMinimalTyp from '../typst/styles/slides-minimal.typ?raw';
import admonitionsTyp from '../typst/admonitions.typ?raw';

type CompileRequest = {
	type: 'compile';
	id: string;
	mainTypst: string;
	images?: Record<string, Uint8Array<ArrayBuffer>>;
	format?: 'pdf' | 'vector';
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
			vector: ArrayBuffer;
			diagnostics: string[];
	  }
	| {
			type: 'compile-result';
			id: string;
			ok: false;
			error: string;
			diagnostics: string[];
	  };

const ctx: DedicatedWorkerGlobalScope = self as unknown as DedicatedWorkerGlobalScope;

let compilerPromise: Promise<TypstCompiler> | null = null;
let compileQueue: Promise<void> = Promise.resolve();

// All fonts served from same-origin /fonts/* (bundled at build time by the
// `md2pdf-bundle-fonts` Vite plugin in vite.config.ts). No external CDNs.
// We also include the DejaVu mono fonts typst.ts would otherwise pull
// from jsdelivr via `assets: ['text']`, and skip that asset bundle below.
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

const EMOJI_FONTS: string[] = ['/fonts/NotoColorEmoji.ttf'];

let emojiLoaded = false;

async function upgradeCompiler(needEmoji: boolean) {
	if (!needEmoji || emojiLoaded) return;
	emojiLoaded = true;

	console.log('md2pdf - Upgrading compiler (Emoji)...');

	const fontsToLoad = [...CORE_FONTS, ...EMOJI_FONTS];

	const newCompiler = createTypstCompiler();
	await newCompiler.init({
		getModule: () => typstCompilerWasmUrl,
		beforeBuild: [loadFonts(fontsToLoad, { assets: false })]
	});
	newCompiler.addSource('/styles/modern-tech.typ', modernTechTyp);
	newCompiler.addSource('/styles/redbook-knowledge.typ', redbookKnowledgeTyp);
	newCompiler.addSource('/styles/redbook-dark.typ', redbookDarkTyp);
	newCompiler.addSource('/styles/redbook-minimalist.typ', redbookMinimalistTyp);
	newCompiler.addSource('/styles/redbook-modern.typ', redbookModernTyp);
	newCompiler.addSource('/styles/redbook-forest.typ', redbookForestTyp);
	newCompiler.addSource('/styles/redbook-blueprint.typ', redbookBlueprintTyp);
	newCompiler.addSource('/styles/redbook-clean.typ', redbookCleanTyp);
	newCompiler.addSource('/styles/redbook-typography.typ', redbookTypographyTyp);
	newCompiler.addSource('/styles/slides-modern.typ', slidesModernTyp);
	newCompiler.addSource('/styles/slides-dark.typ', slidesDarkTyp);
	newCompiler.addSource('/styles/slides-minimal.typ', slidesMinimalTyp);
	newCompiler.addSource('/admonitions.typ', admonitionsTyp);

	// Swap the compiler promise
	compilerPromise = Promise.resolve(newCompiler);
	console.log('md2pdf - Compiler upgraded successfully.');
}

function getCompiler(): Promise<TypstCompiler> {
	if (compilerPromise) return compilerPromise;

	compilerPromise = (async () => {
		const compiler = createTypstCompiler();
		await compiler.init({
			getModule: () => typstCompilerWasmUrl,
			beforeBuild: [loadFonts(CORE_FONTS, { assets: false })]
		});
		compiler.addSource('/styles/modern-tech.typ', modernTechTyp);
		compiler.addSource('/styles/redbook-knowledge.typ', redbookKnowledgeTyp);
		compiler.addSource('/styles/redbook-dark.typ', redbookDarkTyp);
		compiler.addSource('/styles/redbook-minimalist.typ', redbookMinimalistTyp);
		compiler.addSource('/styles/redbook-modern.typ', redbookModernTyp);
		compiler.addSource('/styles/redbook-forest.typ', redbookForestTyp);
		compiler.addSource('/styles/redbook-blueprint.typ', redbookBlueprintTyp);
		compiler.addSource('/styles/redbook-clean.typ', redbookCleanTyp);
		compiler.addSource('/styles/redbook-typography.typ', redbookTypographyTyp);
		compiler.addSource('/styles/slides-modern.typ', slidesModernTyp);
		compiler.addSource('/styles/slides-dark.typ', slidesDarkTyp);
		compiler.addSource('/styles/slides-minimal.typ', slidesMinimalTyp);
		compiler.addSource('/admonitions.typ', admonitionsTyp);
		return compiler;
	})();

	return compilerPromise;
}

async function compileTypst(
	mainTypst: string,
	images: Record<string, Uint8Array<ArrayBuffer>> = {},
	format: 'pdf' | 'vector' = 'pdf'
): Promise<{ result: Uint8Array; diagnostics: string[] }> {
	// Load the emoji font on demand when the document contains emoji.
	const hasEmoji = /[\u{1F000}-\u{1F9FF}\u{2600}-\u{26FF}\u{2700}-\u{27BF}]/u.test(mainTypst);

	if (hasEmoji) {
		await upgradeCompiler(hasEmoji);
	}

	const compiler = await getCompiler();
	compiler.addSource('/main.typ', mainTypst);

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
			const { result, diagnostics } = await compileTypst(message.mainTypst, message.images, fmt);
			const copy = new Uint8Array(result.length);
			copy.set(result);
			const response: CompileResponse = fmt === 'pdf'
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
