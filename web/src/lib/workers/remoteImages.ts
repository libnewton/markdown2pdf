// Remote images for both targets. Typst can only read images from its VFS and
// the HTML renderer only embeds bytes it is handed, so the worker fetches each
// http(s) image the engine lists and keeps the verdict for the session.
//
// If a fetch fails (most commonly CORS) the image becomes a blank stand-in:
// the rest of the document still renders, and the reader is told.

// The same limits the CLI documents. Without them one document can point at a
// hundred URLs, or at a file with no end to it, and spend the tab's memory and
// connections on the author's behalf.
export const MAX_BYTES = 32 * 1024 * 1024;
export const TIMEOUT_MS = 20_000;
export const MAX_IMAGES = 64;
export const MAX_PARALLEL = 6;

/** A 1×1 fully transparent PNG: valid to decode, invisible on the page. */
export const BLANK_PNG = new Uint8Array([
	0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
	0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
	0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
	0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
	0x42, 0x60, 0x82,
]);

/** `null` is a URL that failed: known, so not fetched again. */
const cache = new Map<string, Uint8Array | null>();
const inflight = new Map<string, Promise<void>>();

async function fetchDirect(url: string): Promise<Uint8Array | null> {
	const resp = await fetch(url, {
		mode: 'cors',
		// A document must never be able to make a request that carries the
		// reader's cookies, even to this app's own origin.
		credentials: 'omit',
		redirect: 'follow',
		signal: AbortSignal.timeout(TIMEOUT_MS),
	});
	if (!resp.ok) return null;
	const declared = Number(resp.headers.get('content-length'));
	if (declared > MAX_BYTES) return null;
	const buf = await resp.arrayBuffer();
	if (buf.byteLength > MAX_BYTES) return null;
	return new Uint8Array(buf);
}

function fetchOne(url: string): Promise<void> {
	let pending = inflight.get(url);
	if (!pending) {
		pending = fetchDirect(url)
			.catch(() => null)
			.then((bytes) => {
				cache.set(url, bytes);
				inflight.delete(url);
			});
		inflight.set(url, pending);
	}
	return pending;
}

export type RemoteLookup = {
	/** Bytes by the alias the engine gave, blank for a failed URL. */
	images: [alias: string, bytes: Uint8Array][];
	/** URLs that could not be fetched. */
	failed: string[];
	/** URLs with no verdict yet. */
	missing: string[];
};

/** What is known about `rows` (`[url, alias]` pairs), without touching the network. */
export function lookupRemotes(rows: readonly (readonly [string, string])[]): RemoteLookup {
	const result: RemoteLookup = { images: [], failed: [], missing: [] };
	for (const [url, alias] of rows) {
		if (!cache.has(url)) {
			result.missing.push(url);
			continue;
		}
		const bytes = cache.get(url);
		if (!bytes) result.failed.push(url);
		result.images.push([alias, bytes ?? BLANK_PNG]);
	}
	return result;
}

/**
 * Fetch the URLs that have no verdict yet, a few at a time. Firing every URL
 * at once is what turns a document with a long image list into a stall.
 *
 * Resolves to true when any of them reached a verdict — a failure counts,
 * because it now produces a placeholder the document should be re-rendered
 * with.
 */
export async function fetchRemotes(urls: readonly string[]): Promise<boolean> {
	const queue = urls.filter((url) => !cache.has(url)).slice(0, MAX_IMAGES);
	let next = 0;
	await Promise.all(
		Array.from({ length: Math.min(MAX_PARALLEL, queue.length) }, async () => {
			while (next < queue.length) await fetchOne(queue[next++]);
		}),
	);
	return queue.length > 0;
}
