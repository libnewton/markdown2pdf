import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import { SvelteKitPWA } from '@vite-pwa/sveltekit';
// @ts-ignore -- node builtins are present at vite build time
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
// @ts-ignore
import { dirname, join } from 'node:path';
// @ts-ignore
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Bundle the `md2pdf` Typst package (engine.wasm + lib.typ + styles + vendored
// mitex/mmdr) into static/md2pdf/ so the typst.ts worker can load it. This is
// the single Markdown-processing codebase, shared with the CLI.
function copyMd2pdfPackage(): Plugin {
	const src = join(__dirname, '../package');
	const dest = join(__dirname, 'static/md2pdf');
	return {
		name: 'md2pdf-copy-package',
		buildStart() {
			if (!existsSync(src)) {
				this.warn('md2pdf package not found at ../package');
				return;
			}
			rmSync(dest, { recursive: true, force: true });
			mkdirSync(dest, { recursive: true });
			// manifest = files the worker registers upfront (.typ source + .wasm
			// plugins). Twemoji .svg files are copied too but fetched lazily by
			// the worker (only the glyphs a document actually uses).
			const manifest: string[] = [];
			let svgCount = 0;
			const walk = (dir: string, base: string) => {
				for (const entry of readdirSync(dir, { withFileTypes: true })) {
					const rel = base ? `${base}/${entry.name}` : entry.name;
					if (entry.isDirectory()) {
						walk(join(dir, entry.name), rel);
						continue;
					}
					const isTyp = entry.name.endsWith('.typ');
					const isWasm = entry.name.endsWith('.wasm');
					const isSvg = entry.name.endsWith('.svg');
					// Fonts are fetched on demand too — the preview pane uses them,
					// and an HTML download embeds them.
					const isFont = entry.name.endsWith('.woff2');
					if (!isTyp && !isWasm && !isSvg && !isFont) continue;
					mkdirSync(dirname(join(dest, rel)), { recursive: true });
					copyFileSync(join(dir, entry.name), join(dest, rel));
					if (isTyp || isWasm) manifest.push(rel);
					else if (isSvg) svgCount++;
				}
			};
			walk(src, '');
			writeFileSync(join(dest, 'manifest.json'), JSON.stringify(manifest));
			this.info(
				`copied md2pdf package (${manifest.length} core + ${svgCount} twemoji) → static/md2pdf`,
			);
		},
	};
}

// The CJK faces are ~8 MB each and most documents never touch them, so they
// are not in the repo and not in the precache — the worker fetches one only
// once a document actually contains CJK. Downloaded here once, on the first
// build, like the rest of static/fonts.
const CJK_FONTS: Record<string, string> = {
	'NotoSansSC-Regular.otf':
		'https://cdn.jsdelivr.net/gh/notofonts/noto-cjk@main/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf',
	'NotoSansSC-Bold.otf':
		'https://cdn.jsdelivr.net/gh/notofonts/noto-cjk@main/Sans/SubsetOTF/SC/NotoSansSC-Bold.otf',
	'NotoSansKR-Regular.otf':
		'https://cdn.jsdelivr.net/gh/notofonts/noto-cjk@main/Sans/SubsetOTF/KR/NotoSansKR-Regular.otf',
	'NotoSansKR-Bold.otf':
		'https://cdn.jsdelivr.net/gh/notofonts/noto-cjk@main/Sans/SubsetOTF/KR/NotoSansKR-Bold.otf',
};

// Copy the fonts md2pdf needs at runtime from the repo's shared `fonts/` dir
// into static/fonts/, so the typst.ts worker is fully offline — no CDN calls.
function bundleFonts(): Plugin {
	const src = join(__dirname, '../fonts');
	const dest = join(__dirname, 'static/fonts');
	return {
		name: 'md2pdf-bundle-fonts',
		async buildStart() {
			if (!existsSync(src)) {
				this.warn('fonts not found at ../fonts');
				return;
			}
			// Keep any CJK face already downloaded rather than re-fetching 16 MB.
			const keep = new Map<string, Buffer>();
			for (const name of Object.keys(CJK_FONTS)) {
				const at = join(dest, name);
				if (existsSync(at)) keep.set(name, readFileSync(at));
			}
			rmSync(dest, { recursive: true, force: true });
			mkdirSync(dest, { recursive: true });
			let count = 0;
			for (const entry of readdirSync(src, { withFileTypes: true })) {
				if (!entry.isFile() || !/\.(ttf|otf)$/i.test(entry.name)) continue;
				copyFileSync(join(src, entry.name), join(dest, entry.name));
				count++;
			}
			for (const [name, bytes] of keep) writeFileSync(join(dest, name), bytes);

			for (const [name, url] of Object.entries(CJK_FONTS)) {
				if (keep.has(name)) continue;
				try {
					const resp = await fetch(url);
					if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
					writeFileSync(join(dest, name), Buffer.from(await resp.arrayBuffer()));
					this.info(`downloaded ${name} → static/fonts`);
				} catch (error) {
					// Not fatal: everything but CJK still renders, and the next
					// build with a network will pick it up.
					this.warn(`could not download ${name} (${error}); CJK text will not render`);
				}
			}
			this.info(`copied ${count} font(s) → static/fonts`);
		},
	};
}

export default defineConfig({
	plugins: [
		copyMd2pdfPackage(),
		bundleFonts(),
		sveltekit(),
		SvelteKitPWA({
			registerType: 'autoUpdate',
			workbox: {
				// The twemoji mirror is ~3700 files and 17 MB, fetched one glyph
				// at a time only by documents that use emoji. Precaching it would
				// mean downloading all of it to install the app.
				// Same argument for the CJK faces: ~26 MB, fetched only by the
				// documents that need them.
				globIgnores: ['**/md2pdf/twemoji/**', '**/fonts/NotoSans[SK][CR]-*'],
				// engine.wasm and the Typst compiler are past the 2 MB default.
				maximumFileSizeToCacheInBytes: 8 * 1024 * 1024,
			},
			manifest: {
				name: 'md2pdf',
				short_name: 'md2pdf',
				description: 'Markdown to PDF Professional Export Tool',
				theme_color: '#ffffff',
				icons: [
					{
						src: 'favicon-16x16.png',
						sizes: '16x16',
						type: 'image/png',
					},
					{
						src: 'favicon-32x32.png',
						sizes: '32x32',
						type: 'image/png',
					},
					{
						src: 'logo.png',
						sizes: '183x100',
						type: 'image/png',
					},
					{
						src: 'apple-touch-icon.png',
						sizes: '180x180',
						type: 'image/png',
					},
					{
						src: 'square.png',
						sizes: '240x240',
						type: 'image/png',
						purpose: 'any maskable',
					},
				],
			},
		}),
	],
	worker: {
		format: 'es',
	},
});
