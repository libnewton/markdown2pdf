import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import { SvelteKitPWA } from '@vite-pwa/sveltekit';
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
					if (!isTyp && !isWasm && !isSvg) continue;
					mkdirSync(dirname(join(dest, rel)), { recursive: true });
					copyFileSync(join(dir, entry.name), join(dest, rel));
					if (isTyp || isWasm) manifest.push(rel);
					else svgCount++;
				}
			};
			walk(src, '');
			writeFileSync(join(dest, 'manifest.json'), JSON.stringify(manifest));
			this.info(
				`copied md2pdf package (${manifest.length} core + ${svgCount} twemoji) → static/md2pdf`
			);
		}
	};
}

// Copy the fonts md2pdf needs at runtime from the repo's shared `fonts/` dir
// into static/fonts/, so the typst.ts worker is fully offline — no CDN calls.
function bundleFonts(): Plugin {
	const src = join(__dirname, '../fonts');
	const dest = join(__dirname, 'static/fonts');
	return {
		name: 'md2pdf-bundle-fonts',
		buildStart() {
			if (!existsSync(src)) {
				this.warn('fonts not found at ../fonts');
				return;
			}
			rmSync(dest, { recursive: true, force: true });
			mkdirSync(dest, { recursive: true });
			let count = 0;
			for (const entry of readdirSync(src, { withFileTypes: true })) {
				if (!entry.isFile() || !/\.(ttf|otf)$/i.test(entry.name)) continue;
				copyFileSync(join(src, entry.name), join(dest, entry.name));
				count++;
			}
			this.info(`copied ${count} font(s) → static/fonts`);
		}
	};
}

export default defineConfig({
	plugins: [
		copyMd2pdfPackage(),
		bundleFonts(),
		sveltekit(),
		SvelteKitPWA({
			registerType: 'autoUpdate',
			manifest: {
				name: 'md2pdf',
				short_name: 'md2pdf',
				description: 'Markdown to PDF Professional Export Tool',
				theme_color: '#ffffff',
				icons: [
					{
						src: 'favicon-16x16.png',
						sizes: '16x16',
						type: 'image/png'
					},
					{
						src: 'favicon-32x32.png',
						sizes: '32x32',
						type: 'image/png'
					},
					{
						src: 'logo.png',
						sizes: '183x100',
						type: 'image/png'
					},
					{
						src: 'apple-touch-icon.png',
						sizes: '180x180',
						type: 'image/png'
					},
					{
						src: 'square.png',
						sizes: '240x240',
						type: 'image/png',
						purpose: 'any maskable'
					}
				]
			}
		})
	],
	worker: {
		format: 'es'
	}
});
