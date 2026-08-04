import { describe, expect, it } from 'vitest';
import { buildAssetBundle, parseKeyedSources } from '../src/lib/workers/assetBundle';

const bytes = (s: string) => new TextEncoder().encode(s);
const text = (b: Uint8Array) => new TextDecoder().decode(b);

describe('buildAssetBundle', () => {
	it('concatenates the payload and describes it by byte length', () => {
		const { manifest, blob } = buildAssetBundle([
			['images/a.png', bytes('abc')],
			['twemoji/1f600.svg', bytes('de')]
		]);
		expect(manifest).toBe('images/a.png\t3\ntwemoji/1f600.svg\t2\n');
		expect(text(blob)).toBe('abcde');
	});

	it('measures bytes, not characters, so multi-byte assets stay aligned', () => {
		const { manifest, blob } = buildAssetBundle([
			['a', bytes('é😀')],
			['b', bytes('x')]
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

describe('parseKeyedSources', () => {
	it('splits key and source and restores newlines', () => {
		expect(parseKeyedSources('mermaid/abc.svg\tgraph LR\\nA-->B\n')).toEqual([
			{ key: 'mermaid/abc.svg', source: 'graph LR\nA-->B' }
		]);
	});

	it('restores literal backslashes without eating the next escape', () => {
		// Wire `a\\nb` came from a source holding a backslash followed by `n`.
		expect(parseKeyedSources('k\ta\\\\nb\n')[0].source).toBe('a\\nb');
		// Wire `a\\\n b` came from a backslash followed by a real newline.
		expect(parseKeyedSources('k\ta\\\\\\nb\n')[0].source).toBe('a\\\nb');
	});

	it('leaves an unknown escape alone', () => {
		expect(parseKeyedSources('k\ta\\tb\n')[0].source).toBe('a\\tb');
	});

	it('ignores blank lines', () => {
		expect(parseKeyedSources('\n')).toEqual([]);
		expect(parseKeyedSources('')).toEqual([]);
	});
});
