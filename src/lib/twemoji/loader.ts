import { get as getEmoji } from 'node-emoji';

// Mirror of the regex in src/lib/pipeline/plugins/remark-twemoji.ts.
// Kept in sync manually — both must produce the same codepoint set for a given source.
const EMOJI_REGEX =
	/(?:\p{Extended_Pictographic}(?:‍\p{Extended_Pictographic})*(?:️)?|\p{Regional_Indicator}{2})/gu;
const SHORTCODE_RE = /:([a-z0-9_+-]+):/gi;

function toCodepoint(emoji: string): string {
	const parts: string[] = [];
	for (const ch of emoji) {
		const cp = ch.codePointAt(0);
		if (cp === undefined) continue;
		if (cp === 0xfe0f) continue;
		parts.push(cp.toString(16));
	}
	return parts.join('-');
}

// Expand :shortcode: → unicode emoji so the unicode regex picks them up.
// Same mapping as remark-emoji-shortcodes.ts; unknown codes are left intact.
function expandShortcodes(markdown: string): string {
	return markdown.replace(SHORTCODE_RE, (full, name) => {
		const emoji = getEmoji(name);
		if (!emoji || emoji === `:${name}:`) return full;
		return emoji;
	});
}

export function collectTwemojiCodepoints(markdown: string): Set<string> {
	const expanded = expandShortcodes(markdown);
	const out = new Set<string>();
	const matches = expanded.matchAll(EMOJI_REGEX);
	for (const m of matches) out.add(toCodepoint(m[0]));
	return out;
}

const cache = new Map<string, Uint8Array | null>();

async function fetchOne(codepoint: string): Promise<Uint8Array | null> {
	if (cache.has(codepoint)) return cache.get(codepoint) ?? null;
	try {
		const resp = await fetch(`/twemoji/${codepoint}.svg`);
		if (!resp.ok) {
			cache.set(codepoint, null);
			return null;
		}
		const buf = await resp.arrayBuffer();
		const bytes = new Uint8Array(buf);
		cache.set(codepoint, bytes);
		return bytes;
	} catch {
		cache.set(codepoint, null);
		return null;
	}
}

/**
 * Given the source markdown, fetch all twemoji SVGs needed by it and return
 * an `images` map keyed by `twemoji/<codepoint>.svg` (the path used in the
 * Typst source).
 */
export async function loadTwemojiImages(
	markdown: string
): Promise<Record<string, Uint8Array>> {
	const codepoints = collectTwemojiCodepoints(markdown);
	const entries = await Promise.all(
		[...codepoints].map(async (cp) => {
			const bytes = await fetchOne(cp);
			return bytes ? ([`twemoji/${cp}.svg`, bytes] as const) : null;
		})
	);
	const out: Record<string, Uint8Array> = {};
	for (const entry of entries) {
		if (entry) out[entry[0]] = entry[1];
	}
	return out;
}
