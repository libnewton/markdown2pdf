import { defineConfig, type Plugin } from 'vite';
import adapter from '@sveltejs/adapter-static';
import { withPwa } from '@vite-pwa/sveltekit';
// @ts-ignore -- node builtins are present at vite build time
import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
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
// once a document actually contains CJK. They are downloaded once into
// .cache/cjk (CI caches that directory) and copied from there on every build.
// Pinned to a release tag so every build ships the same bytes.
const CJK_BASE = 'https://cdn.jsdelivr.net/gh/notofonts/noto-cjk@Sans2.004/Sans/SubsetOTF';
const CJK_FONTS: Record<string, string> = {
	'NotoSansSC-Regular.otf': `${CJK_BASE}/SC/NotoSansSC-Regular.otf`,
	'NotoSansSC-Bold.otf': `${CJK_BASE}/SC/NotoSansSC-Bold.otf`,
	'NotoSansKR-Regular.otf': `${CJK_BASE}/KR/NotoSansKR-Regular.otf`,
	'NotoSansKR-Bold.otf': `${CJK_BASE}/KR/NotoSansKR-Bold.otf`,
};

// Copy the fonts md2pdf needs at runtime from the repo's shared `fonts/` dir
// into static/fonts/, so the typst.ts worker is fully offline — no CDN calls.
function bundleFonts(): Plugin {
	const src = join(__dirname, '../fonts');
	const dest = join(__dirname, 'static/fonts');
	const cache = join(__dirname, '.cache/cjk');
	return {
		name: 'md2pdf-bundle-fonts',
		async buildStart() {
			if (!existsSync(src)) {
				this.warn('fonts not found at ../fonts');
				return;
			}
			rmSync(dest, { recursive: true, force: true });
			mkdirSync(dest, { recursive: true });
			mkdirSync(cache, { recursive: true });
			let count = 0;
			for (const entry of readdirSync(src, { withFileTypes: true })) {
				if (!entry.isFile() || !/\.(ttf|otf)$/i.test(entry.name)) continue;
				copyFileSync(join(src, entry.name), join(dest, entry.name));
				count++;
			}
			for (const [name, url] of Object.entries(CJK_FONTS)) {
				const cached = join(cache, name);
				if (!existsSync(cached)) {
					try {
						const resp = await fetch(url);
						if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
						writeFileSync(cached, Buffer.from(await resp.arrayBuffer()));
						this.info(`downloaded ${name} → .cache/cjk`);
					} catch (error) {
						// Not fatal: everything but CJK still renders, and the next
						// build with a network will pick it up.
						this.warn(`could not download ${name} (${error}); CJK text will not render`);
						continue;
					}
				}
				copyFileSync(cached, join(dest, name));
			}
			this.info(`copied ${count} font(s) → static/fonts`);
		},
	};
}

// Unit tests import modules directly and never need the copied assets.
// @ts-ignore -- node globals are present at config time
const assetPlugins = process.env.VITEST ? [] : [copyMd2pdfPackage(), bundleFonts()];

export default defineConfig({
	plugins: [
		...assetPlugins,
		// Wraps the sveltekit() plugin.
		withPwa(
			{
				adapter: adapter(),
				alias: { $lib: 'src/lib' },
				prerender: { handleHttpError: 'fail' },
			},
			{
				registerType: 'autoUpdate',
				// engine.wasm and the Typst compiler are past the 2 MB default.
				maximumFileSizeToCacheInBytes: 8 * 1024 * 1024,
				workbox: {
					// The twemoji mirror is ~3700 files and 17 MB, fetched one glyph
					// at a time only by documents that use emoji. Precaching it would
					// mean downloading all of it to install the app.
					// Same argument for the CJK faces: ~26 MB, fetched only by the
					// documents that need them.
					globIgnores: ['**/md2pdf/twemoji/**', '**/fonts/NotoSans[SK][CR]-*'],
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
			},
		),
	],
	worker: {
		format: 'es',
	},
});
