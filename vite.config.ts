import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import { SvelteKitPWA } from '@vite-pwa/sveltekit';
// @ts-ignore -- node builtins are present at vite build time
import { copyFileSync, existsSync, mkdirSync, readdirSync, statSync, writeFileSync } from 'node:fs';
// @ts-ignore
import { dirname, join } from 'node:path';
// @ts-ignore
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

function copyTwemojiAssets(): Plugin {
	const src = join(__dirname, 'node_modules/twemoji-emojis/vendor/svg');
	const dest = join(__dirname, 'static/twemoji');
	return {
		name: 'md2pdf-copy-twemoji',
		buildStart() {
			if (!existsSync(src)) return;
			try {
				if (existsSync(dest) && statSync(dest).isDirectory()) {
					// Already copied — skip (saves ~17MB of file ops on every start)
					return;
				}
				mkdirSync(dest, { recursive: true });
				for (const f of readdirSync(src)) {
					if (!f.endsWith('.svg')) continue;
					copyFileSync(join(src, f), join(dest, f));
				}
				this.info(`copied twemoji svgs → static/twemoji`);
			} catch (err) {
				this.warn(`twemoji copy failed: ${(err as Error).message}`);
			}
		}
	};
}

// Fonts md2pdf needs at runtime. Downloaded once at dev/build start so we
// never call out to a CDN from the user's browser.
const FONTS_TO_BUNDLE: Array<{ filename: string; url: string }> = [
	{
		filename: 'IBMPlexSans-Regular.ttf',
		url: 'https://cdn.jsdelivr.net/gh/typst/typst-dev-assets@v0.13.1/files/fonts/IBMPlexSans-Regular.ttf'
	},
	{
		filename: 'IBMPlexSans-Bold.ttf',
		url: 'https://cdn.jsdelivr.net/gh/typst/typst-dev-assets@v0.13.1/files/fonts/IBMPlexSans-Bold.ttf'
	},
	{
		filename: 'NewCMMath-Regular.otf',
		url: 'https://cdn.jsdelivr.net/gh/typst/typst-assets@v0.13.1/files/fonts/NewCMMath-Regular.otf'
	},
	{
		filename: 'NewCMMath-Book.otf',
		url: 'https://cdn.jsdelivr.net/gh/typst/typst-assets@v0.13.1/files/fonts/NewCMMath-Book.otf'
	},
	{
		filename: 'NotoColorEmoji.ttf',
		url: 'https://fonts.gstatic.com/s/notocoloremoji/v37/Yq6P-KqIXTD0t4D9z1ESnKM3-HpFab4.ttf'
	},
	// Fonts that typst.ts would otherwise fetch from jsdelivr via assets:['text'].
	// We bundle them so the worker is fully offline.
	{
		filename: 'DejaVuSansMono.ttf',
		url: 'https://cdn.jsdelivr.net/gh/typst/typst-assets@v0.13.1/files/fonts/DejaVuSansMono.ttf'
	},
	{
		filename: 'DejaVuSansMono-Bold.ttf',
		url: 'https://cdn.jsdelivr.net/gh/typst/typst-assets@v0.13.1/files/fonts/DejaVuSansMono-Bold.ttf'
	},
	{
		filename: 'DejaVuSansMono-Oblique.ttf',
		url: 'https://cdn.jsdelivr.net/gh/typst/typst-assets@v0.13.1/files/fonts/DejaVuSansMono-Oblique.ttf'
	},
	{
		filename: 'DejaVuSansMono-BoldOblique.ttf',
		url: 'https://cdn.jsdelivr.net/gh/typst/typst-assets@v0.13.1/files/fonts/DejaVuSansMono-BoldOblique.ttf'
	}
];

function bundleFonts(): Plugin {
	const dest = join(__dirname, 'static/fonts');
	return {
		name: 'md2pdf-bundle-fonts',
		async buildStart() {
			if (!existsSync(dest)) mkdirSync(dest, { recursive: true });
			const missing = FONTS_TO_BUNDLE.filter((f) => !existsSync(join(dest, f.filename)));
			if (missing.length === 0) return;
			this.info(`fetching ${missing.length} font(s) → static/fonts (one-time)`);
			await Promise.all(
				missing.map(async (font) => {
					try {
						const resp = await fetch(font.url);
						if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
						const buf = new Uint8Array(await resp.arrayBuffer());
						writeFileSync(join(dest, font.filename), buf);
						this.info(`  ✓ ${font.filename}`);
					} catch (err) {
						this.warn(
							`  ✗ ${font.filename} (${(err as Error).message}) — typst will be missing this font`
						);
					}
				})
			);
		}
	};
}

export default defineConfig({
	plugins: [
		copyTwemojiAssets(),
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
