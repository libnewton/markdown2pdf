import { deriveNameFromContent } from '#lib/stores/documentStore.svelte.ts';
import type { TypstWorkerClient } from '#lib/workers/typstClient.ts';
import type { ImageMap } from '#lib/workers/compileProtocol.ts';
import type { Theme } from '#lib/stores/settingsStore.svelte.ts';

/** Save `blob` under the document's name. */
function download(blob: Blob, markdown: string, extension: string) {
	const url = URL.createObjectURL(blob);
	const link = document.createElement('a');
	link.href = url;
	link.download =
		(deriveNameFromContent(markdown) || 'document').replace(/[/\\?%*:|"<>]/g, '-') + extension;
	link.click();
	setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

/** Compile first, then save: a failed compile leaves nothing half-open behind. */
export async function savePdf(client: TypstWorkerClient, markdown: string, images: ImageMap) {
	const { pdf } = await client.compilePdf(markdown, images);
	download(new Blob([pdf], { type: 'application/pdf' }), markdown, '.pdf');
}

export async function downloadHtml(
	client: TypstWorkerClient,
	markdown: string,
	images: ImageMap,
	theme: Theme,
) {
	const { html } = await client.renderHtml(markdown, images, true, false, theme);
	download(new Blob([html], { type: 'text/html;charset=utf-8' }), markdown, '.html');
}

export function downloadMarkdown(markdown: string) {
	download(new Blob([markdown], { type: 'text/markdown;charset=utf-8' }), markdown, '.md');
}
