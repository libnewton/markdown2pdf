// Best-effort remote-image loader for the typst worker.
//
// Typst itself can only read images from its in-memory VFS; we pre-fetch each
// http(s) image referenced from the markdown and pass the bytes into the
// `images` map keyed by `remote/<hash>` — matching the alias the md2pdf
// engine emits (its `hash_url` is the same FNV-1a hash as `hashUrl` below).
//
// If a fetch fails (most commonly CORS) we just skip it — the image renders as
// a placeholder, and the user can download it and re-insert it by hand.

const MARKDOWN_IMAGE_REGEX = /!\[[^\]]*]\((https?:\/\/[^)\s]+)(?:\s+[^)]*)?\)/g;

function hashUrl(url: string): string {
	let h = 0x811c9dc5;
	for (let i = 0; i < url.length; i++) {
		h ^= url.charCodeAt(i);
		h = (h + ((h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24))) >>> 0;
	}
	return h.toString(16).padStart(8, '0');
}

const cache = new Map<string, Uint8Array<ArrayBuffer> | null>();

async function fetchDirect(url: string): Promise<Uint8Array<ArrayBuffer> | null> {
	const resp = await fetch(url, { mode: 'cors' });
	if (!resp.ok) return null;
	const buf = await resp.arrayBuffer();
	return new Uint8Array(buf);
}

async function fetchOne(url: string): Promise<Uint8Array<ArrayBuffer> | null> {
	if (cache.has(url)) return cache.get(url) ?? null;
	try {
		const direct = await fetchDirect(url);
		if (direct) {
			cache.set(url, direct);
			return direct;
		}
	} catch {
		/* a blocked or broken URL is remembered as a miss */
	}
	cache.set(url, null);
	return null;
}

export function collectRemoteImageUrls(markdown: string): string[] {
	const urls = new Set<string>();
	for (const m of markdown.matchAll(MARKDOWN_IMAGE_REGEX)) {
		urls.add(m[1]);
	}
	return [...urls];
}

/**
 * The already-fetched subset, without touching the network. The live-preview
 * loop uses this so a compile never waits on a slow (or hanging) image fetch:
 * it renders with what is cached and recompiles once `prefetchRemoteImages`
 * reports something new.
 */
export function cachedRemoteImages(markdown: string): {
	images: Record<string, Uint8Array<ArrayBuffer>>;
	missing: string[];
} {
	const images: Record<string, Uint8Array<ArrayBuffer>> = {};
	const missing: string[] = [];
	for (const url of collectRemoteImageUrls(markdown)) {
		if (!cache.has(url)) {
			missing.push(url);
			continue;
		}
		const bytes = cache.get(url);
		// A cached `null` is a URL that failed — known, so not missing.
		if (bytes) images[`remote/${hashUrl(url)}`] = bytes;
	}
	return { images, missing };
}

/**
 * Fetch the images this document references that are not cached yet.
 * Resolves to true when at least one new image became available, i.e. when a
 * recompile would show something new.
 */
export async function prefetchRemoteImages(urls: string[]): Promise<boolean> {
	if (urls.length === 0) return false;
	const results = await Promise.all(urls.map((url) => fetchOne(url)));
	return results.some((bytes) => bytes !== null);
}
