/**
 * The wire format the engine's `render_html` expects for host-resolved assets:
 * one concatenated byte blob plus a `key<TAB>byte-length` manifest describing
 * how to slice it. The Typst package builds the same pair for the CLI, so both
 * front-ends hand the engine identical input.
 */
export type Asset = readonly [key: string, bytes: Uint8Array];

export function buildAssetBundle(assets: readonly Asset[]): {
	manifest: string;
	blob: Uint8Array;
} {
	const blob = new Uint8Array(assets.reduce((n, [, bytes]) => n + bytes.length, 0));
	let offset = 0;
	let manifest = '';
	for (const [key, bytes] of assets) {
		blob.set(bytes, offset);
		offset += bytes.length;
		manifest += `${key}\t${bytes.length}\n`;
	}
	return { manifest, blob };
}

/**
 * Undo the engine's `\\` and `\n` escaping, which is what lets a multi-line
 * Mermaid diagram travel inside a line-oriented list. One left-to-right pass,
 * so an escaped backslash cannot combine with the `n` after it.
 */
export function unescapeSource(text: string): string {
	return text.replace(/\\(.)/g, (whole, next: string) =>
		next === 'n' ? '\n' : next === '\\' ? '\\' : whole,
	);
}
