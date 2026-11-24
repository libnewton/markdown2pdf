import type { DocumentAssets } from '#lib/storage/documents.ts';

/** One edit, as broadcast to the other tabs that have the same document open. */
export type TabEdit = {
	id: string;
	content: string;
	/** Only when the images changed with this edit. */
	assets?: DocumentAssets;
	/** When the edit was made. The newest edit wins. */
	at: number;
};

const CHANNEL = 'md2pdf-documents';

/**
 * Live sync between tabs of this app. Every edit goes to every other tab; a
 * tab showing the same document takes it if it is newer than its own last
 * edit, so two tabs typing at once settle on the last keystroke instead of
 * silently overwriting each other's autosave.
 */
export function openTabSync(onEdit: (edit: TabEdit) => void): {
	post: (edit: TabEdit) => void;
	close: () => void;
} {
	if (typeof BroadcastChannel === 'undefined') return { post() {}, close() {} };
	const channel = new BroadcastChannel(CHANNEL);
	channel.onmessage = (event: MessageEvent<TabEdit>) => onEdit(event.data);
	return {
		post: (edit) => channel.postMessage(edit),
		close: () => channel.close(),
	};
}
