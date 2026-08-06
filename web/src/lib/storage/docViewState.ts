/**
 * How a document was last being looked at: which preview the reader had open,
 * and where they had dragged the divider between editor and preview. Kept per
 * document, because the answer is a property of the document — a wide table
 * wants a wide preview, a draft wants a wide editor.
 *
 * localStorage rather than IndexedDB: these are two small values that have to
 * be readable synchronously, before the first paint of a document.
 */
export const DOC_VIEW_STATE_KEY = 'md2pdf-doc-view';

export type PreviewMode = 'pages' | 'document';

export interface DocViewState {
	/** The Pages / Web switch above the preview. */
	previewMode?: PreviewMode;
	/** Divider position in split view, as a percentage of the window width. */
	leftPaneWidth?: number;
}

type DocViewStateMap = Record<string, DocViewState>;

/** Absent while prerendering, which is the one place there is nothing to remember. */
function storage(): Storage | null {
	return typeof localStorage === 'undefined' ? null : localStorage;
}

function readAll(): DocViewStateMap {
	const store = storage();
	if (!store) return {};
	try {
		const raw = store.getItem(DOC_VIEW_STATE_KEY);
		if (!raw) return {};
		const parsed: unknown = JSON.parse(raw);
		if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
		return parsed as DocViewStateMap;
	} catch {
		// Unparseable or unreadable storage is the same as none: a remembered
		// layout is never worth failing over.
		return {};
	}
}

function writeAll(map: DocViewStateMap) {
	const store = storage();
	if (!store) return;
	try {
		store.setItem(DOC_VIEW_STATE_KEY, JSON.stringify(map));
	} catch {
		// ignore
	}
}

/** Anything stored by an older version — or by hand — is filtered out here. */
function sanitize(value: unknown): DocViewState {
	if (!value || typeof value !== 'object') return {};
	const { previewMode, leftPaneWidth } = value as DocViewState;
	const state: DocViewState = {};
	if (previewMode === 'pages' || previewMode === 'document') state.previewMode = previewMode;
	if (typeof leftPaneWidth === 'number' && Number.isFinite(leftPaneWidth)) {
		state.leftPaneWidth = leftPaneWidth;
	}
	return state;
}

export function readDocViewState(id: string): DocViewState {
	return sanitize(readAll()[id]);
}

/** Merge into what is stored, so a value the caller left out survives. */
export function updateDocViewState(id: string, patch: DocViewState) {
	const map = readAll();
	const next = sanitize({ ...sanitize(map[id]), ...patch });
	if (Object.keys(next).length === 0) return;
	map[id] = next;
	writeAll(map);
}

export function forgetDocViewState(id: string) {
	const map = readAll();
	if (!(id in map)) return;
	delete map[id];
	writeAll(map);
}

/**
 * Drop what belongs to documents that are gone. Deleting a document clears its
 * entry, but a document removed by another tab — or by clearing IndexedDB
 * alone — leaves one behind, so the list is reconciled on startup too.
 */
export function pruneDocViewState(keepIds: Iterable<string>) {
	const map = readAll();
	const keep = new Set(keepIds);
	let dropped = false;
	for (const id of Object.keys(map)) {
		if (keep.has(id)) continue;
		delete map[id];
		dropped = true;
	}
	if (dropped) writeAll(map);
}
