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
 * Split `key<TAB>escaped-source` lines, undoing the newline escaping the engine
 * applies so a multi-line Mermaid diagram survives a line-oriented list.
 */
export function parseKeyedSources(raw: string): Array<{ key: string; source: string }> {
	return raw
		.split('\n')
		.filter((line) => line !== '')
		.map((line) => {
			const tab = line.indexOf('\t');
			return { key: line.slice(0, tab), source: unescape(line.slice(tab + 1)) };
		});
}

/** Undo the engine's `\\` and `\n` escaping in one left-to-right pass. */
function unescape(text: string): string {
	return text.replace(/\\(.)/g, (whole, next: string) =>
		next === 'n' ? '\n' : next === '\\' ? '\\' : whole
	);
}
