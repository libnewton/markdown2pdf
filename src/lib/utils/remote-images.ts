// Best-effort remote-image loader for the typst worker.
//
// Typst itself can only read images from its in-memory VFS; we pre-fetch each
// http(s) image referenced from the markdown and pass the bytes into the
// `images` map keyed by `remote/<hash>` — matching the alias produced by
// renderImage() in markdownToTypst.ts.
//
// If a fetch fails (most commonly CORS) we just skip it; the user can
// download manually and re-insert via the existing image-paste flow.

const MARKDOWN_IMAGE_REGEX = /!\[[^\]]*]\((https?:\/\/[^)\s]+)(?:\s+[^)]*)?\)/g;

function hashUrl(url: string): string {
	let h = 0x811c9dc5;
	for (let i = 0; i < url.length; i++) {
		h ^= url.charCodeAt(i);
		h = (h + ((h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24))) >>> 0;
	}
	return h.toString(16).padStart(8, '0');
}

const cache = new Map<string, Uint8Array | null>();

async function fetchDirect(url: string): Promise<Uint8Array | null> {
	const resp = await fetch(url, { mode: 'cors' });
	if (!resp.ok) return null;
	const buf = await resp.arrayBuffer();
	return new Uint8Array(buf);
}

async function fetchViaProxy(url: string, proxy: string): Promise<Uint8Array | null> {
	const sep = proxy.includes('?') ? '&' : '?';
	const target = `${proxy}${sep}url=${encodeURIComponent(url)}`;
	const resp = await fetch(target);
	if (!resp.ok) return null;
	const buf = await resp.arrayBuffer();
	return new Uint8Array(buf);
}

async function fetchOne(url: string, proxy: string): Promise<Uint8Array | null> {
	if (cache.has(url)) return cache.get(url) ?? null;
	try {
		const direct = await fetchDirect(url);
		if (direct) {
			cache.set(url, direct);
			return direct;
		}
	} catch {
		/* fall through to proxy */
	}
	if (proxy) {
		try {
			const viaProxy = await fetchViaProxy(url, proxy);
			if (viaProxy) {
				cache.set(url, viaProxy);
				return viaProxy;
			}
		} catch {
			/* give up */
		}
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

export async function loadRemoteImages(
	markdown: string,
	corsProxy = ''
): Promise<Record<string, Uint8Array>> {
	const urls = collectRemoteImageUrls(markdown);
	const entries = await Promise.all(
		urls.map(async (url) => {
			const bytes = await fetchOne(url, corsProxy);
			return bytes ? ([`remote/${hashUrl(url)}`, bytes] as const) : null;
		})
	);
	const out: Record<string, Uint8Array> = {};
	for (const e of entries) {
		if (e) out[e[0]] = e[1];
	}
	return out;
}
