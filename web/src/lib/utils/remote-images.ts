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

// The same limits the CLI documents. Without them one document can point at a
// hundred URLs, or at a file with no end to it, and spend the tab's memory and
// connections on the author's behalf.
const MAX_BYTES = 32 * 1024 * 1024;
const TIMEOUT_MS = 20_000;
const MAX_IMAGES = 64;
const MAX_PARALLEL = 6;

async function fetchDirect(url: string): Promise<Uint8Array<ArrayBuffer> | null> {
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

	const frontmatter = markdown.match(
		/^(?:\uFEFF)?---\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)(?:\r?\n|$)/,
	)?.[1];
	if (frontmatter) {
		const coverImage =
			/^(?:cover-image|cover_image):[ \t]*(?:"(https?:\/\/[^"\r\n]+)"|'(https?:\/\/[^'\r\n]+)'|(https?:\/\/\S+?))(?:[ \t]+#.*)?[ \t]*$/gim;
		for (const match of frontmatter.matchAll(coverImage)) {
			urls.add(match[1] ?? match[2] ?? match[3]);
		}
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
	failed: string[];
} {
	const images: Record<string, Uint8Array<ArrayBuffer>> = {};
	const missing: string[] = [];
	const failed: string[] = [];
	for (const url of collectRemoteImageUrls(markdown)) {
		if (!cache.has(url)) {
			missing.push(url);
			continue;
		}
		const bytes = cache.get(url);
		// A cached `null` is a URL that failed — known, so not missing.
		if (bytes) {
			images[`remote/${hashUrl(url)}`] = bytes;
		} else {
			// Typst treats a missing `image()` as fatal, so one unreachable URL
			// used to take the whole paged preview down with it — silently. A
			// blank stand-in keeps the rest of the document renderable, which
			// is what the CLI has always done.
			images[`remote/${hashUrl(url)}`] = BLANK_PNG;
			failed.push(url);
		}
	}
	return { images, missing, failed };
}

/** A 1×1 fully transparent PNG: valid to decode, invisible on the page. */
const BLANK_PNG = new Uint8Array([
	0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
	0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
	0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
	0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
	0x42, 0x60, 0x82,
]);

/**
 * Fetch the images this document references that are not cached yet.
 *
 * Resolves to true when any of them reached a verdict — a failure counts,
 * because a failure now produces a placeholder, and without a recompile the
 * document would keep the missing image that Typst refuses to render.
 */
export async function prefetchRemoteImages(urls: string[]): Promise<boolean> {
	if (urls.length === 0) return false;
	const queue = urls.slice(0, MAX_IMAGES);
	let next = 0;
	// A few at a time. Firing every URL at once is what turns a document with a
	// long image list into a stall.
	await Promise.all(
		Array.from({ length: Math.min(MAX_PARALLEL, queue.length) }, async () => {
			while (next < queue.length) await fetchOne(queue[next++]);
		}),
	);
	// Every queued URL is in the cache now, so the next pass has an answer for
	// each and this cannot loop.
	return queue.length > 0;
}
