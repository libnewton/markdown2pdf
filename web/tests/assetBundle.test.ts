import { describe, expect, it } from 'vitest';
import { collectRemoteImageUrls } from '../src/lib/utils/remote-images';
import { buildAssetBundle, unescapeSource } from '../src/lib/workers/assetBundle';

const bytes = (s: string) => new TextEncoder().encode(s);
const text = (b: Uint8Array) => new TextDecoder().decode(b);

describe('buildAssetBundle', () => {
	it('concatenates the payload and describes it by byte length', () => {
		const { manifest, blob } = buildAssetBundle([
			['images/a.png', bytes('abc')],
			['twemoji/1f600.svg', bytes('de')],
		]);
		expect(manifest).toBe('images/a.png\t3\ntwemoji/1f600.svg\t2\n');
		expect(text(blob)).toBe('abcde');
	});

	it('measures bytes, not characters, so multi-byte assets stay aligned', () => {
		const { manifest, blob } = buildAssetBundle([
			['a', bytes('é😀')],
			['b', bytes('x')],
		]);
		// 2 bytes for é + 4 for the emoji.
		expect(manifest).toBe('a\t6\nb\t1\n');
		expect(blob.length).toBe(7);
		expect(text(blob.slice(0, 6))).toBe('é😀');
	});

	it('produces an empty bundle for no assets', () => {
		const { manifest, blob } = buildAssetBundle([]);
		expect(manifest).toBe('');
		expect(blob.length).toBe(0);
	});
});

describe('unescapeSource', () => {
	it('restores newlines', () => {
		expect(unescapeSource('graph LR\\nA-->B')).toBe('graph LR\nA-->B');
	});

	it('restores literal backslashes without eating the next escape', () => {
		// Wire `a\\nb` came from a source holding a backslash followed by `n`.
		expect(unescapeSource('a\\\\nb')).toBe('a\\nb');
		// Wire `a\\\n b` came from a backslash followed by a real newline.
		expect(unescapeSource('a\\\\\\nb')).toBe('a\\\nb');
	});

	it('leaves an unknown escape alone', () => {
		expect(unescapeSource('a\\tb')).toBe('a\\tb');
	});

	it('handles an empty source', () => {
		expect(unescapeSource('')).toBe('');
	});
});

describe('collectRemoteImageUrls', () => {
	it('includes bare cover image URLs and deduplicates body references', () => {
		const markdown = `---
cover_image: "https://e.com/cover.png" # full bleed
---

![](https://e.com/body.png)
![](https://e.com/cover.png)`;

		expect(collectRemoteImageUrls(markdown)).toEqual([
			'https://e.com/body.png',
			'https://e.com/cover.png',
		]);
	});

	it('does not treat other bare frontmatter URLs as images', () => {
		const markdown = `---
cover-logo: https://e.com/logo.png
header-left: https://e.com/header.png
---`;

		expect(collectRemoteImageUrls(markdown)).toEqual([]);
	});
});
