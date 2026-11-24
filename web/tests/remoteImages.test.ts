import { describe, expect, it, vi } from 'vitest';

const png = new Uint8Array([1, 2, 3]);

async function load() {
	vi.resetModules();
	return import('#lib/workers/remoteImages.ts');
}

describe('remote images', () => {
	it('has no verdict for a URL until it was fetched', async () => {
		const { lookupRemotes } = await load();
		expect(lookupRemotes([['https://e.com/a.png', 'remote/a']])).toEqual({
			images: [],
			failed: [],
			missing: ['https://e.com/a.png'],
		});
	});

	it('maps fetched bytes to the alias the engine gave, and a failure to a blank', async () => {
		const { lookupRemotes, fetchRemotes, BLANK_PNG } = await load();
		vi.stubGlobal(
			'fetch',
			vi.fn(async (url: string) =>
				url.includes('ok') ? new Response(png) : new Response(null, { status: 404 }),
			),
		);
		const rows = [
			['https://e.com/ok.png', 'remote/1'],
			['https://e.com/gone.png', 'remote/2'],
		] as const;

		expect(await fetchRemotes(rows.map(([url]) => url))).toBe(true);
		const known = lookupRemotes(rows);
		expect(known.missing).toEqual([]);
		expect(known.failed).toEqual(['https://e.com/gone.png']);
		expect(known.images).toEqual([
			['remote/1', png],
			['remote/2', BLANK_PNG],
		]);
		// Every URL has a verdict now, so asking again fetches nothing.
		expect(await fetchRemotes(rows.map(([url]) => url))).toBe(false);
		expect(fetch).toHaveBeenCalledTimes(2);
		vi.unstubAllGlobals();
	});

	it('fetches at most MAX_IMAGES, MAX_PARALLEL at a time, without cookies', async () => {
		const { fetchRemotes, MAX_IMAGES, MAX_PARALLEL } = await load();
		let active = 0;
		let peak = 0;
		const fetchMock = vi.fn(async (_url: string, init?: RequestInit) => {
			expect(init?.credentials).toBe('omit');
			active++;
			peak = Math.max(peak, active);
			await new Promise((resolve) => setTimeout(resolve, 1));
			active--;
			return new Response(png);
		});
		vi.stubGlobal('fetch', fetchMock);
		const urls = Array.from({ length: MAX_IMAGES + 10 }, (_, i) => `https://e.com/${i}.png`);

		await fetchRemotes(urls);
		expect(fetchMock).toHaveBeenCalledTimes(MAX_IMAGES);
		expect(peak).toBe(MAX_PARALLEL);
		vi.unstubAllGlobals();
	});

	it('refuses a response larger than the limit', async () => {
		const { fetchRemotes, lookupRemotes, MAX_BYTES } = await load();
		vi.stubGlobal(
			'fetch',
			vi.fn(
				async () => new Response(png, { headers: { 'content-length': String(MAX_BYTES + 1) } }),
			),
		);
		await fetchRemotes(['https://e.com/huge.png']);
		expect(lookupRemotes([['https://e.com/huge.png', 'remote/h']]).failed).toEqual([
			'https://e.com/huge.png',
		]);
		vi.unstubAllGlobals();
	});
});
