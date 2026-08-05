import {
	saveDocument,
	getDocument,
	listDocuments,
	deleteDocument as deleteDocFromDB,
	type SavedDocument,
	type SavedDocumentAsset,
	type DocumentCreationSource,
} from '$lib/storage/documents';

export type SaveStatus = 'saved' | 'saving';

/**
 * How long typing has to pause before the document is written to IndexedDB.
 * Long enough that a burst of typing writes once rather than every second;
 * `flushPendingSave` covers tab-hide and document switches, so nothing is
 * lost by waiting.
 */
export const AUTOSAVE_DEBOUNCE_MS = 2000;
type InitOptions = {
	restoreCurrent?: boolean;
};

let currentDocId = $state<string | null>(null);
let saveStatus = $state<SaveStatus>('saved');
let recentDocuments = $state<SavedDocument[]>([]);
let isTransitioningDocument = $state(false);

let saveTimer: ReturnType<typeof setTimeout> | null = null;
let hasLoadedSessionCurrent = false;
let pendingSave: {
	id: string;
	content: string;
	assets?: Record<string, SavedDocumentAsset>;
} | null = null;

export function deriveNameFromContent(content: string): string {
	// We only need the title / H1 / first body line, all of which live near
	// the top of the document. Operating on a head slice keeps this O(1) in
	// document size — important because DocumentMenu re-derives the name on
	// every keystroke while the document is still unnamed.
	const head = content.length > 4096 ? content.slice(0, 4096) : content;
	// Try frontmatter title first
	const fmMatch = head.match(/^---\s*\n[\s\S]*?title\s*:\s*["']?(.+?)["']?\s*\n[\s\S]*?---/m);
	if (fmMatch) return fmMatch[1].trim().slice(0, 50);
	// Try H1 heading
	const match = head.match(/^#\s+(.+)$/m);
	if (match) return match[1].trim().slice(0, 50);
	// Fallback to first non-frontmatter line
	const body = head.replace(/^---\s*\n[\s\S]*?\n---\s*\n?/, '').trim();
	const firstLine = body.split('\n', 1)[0]?.trim();
	if (firstLine) return firstLine.slice(0, 50);
	return '';
}

export function isLegacyImplicitBlankDocument(doc: SavedDocument): boolean {
	return doc.creationSource === undefined && doc.content.trim() === '' && doc.name === '';
}

export function isBrokenTemplateDocument(doc: SavedDocument): boolean {
	return doc.creationSource === 'template' && doc.content.trim() === '';
}

function setCurrentDocument(id: string | null, persistSession: boolean) {
	currentDocId = id;
	if (persistSession) {
		if (id) {
			sessionStorage.setItem('md2pdf-current-doc-id', id);
		} else {
			sessionStorage.removeItem('md2pdf-current-doc-id');
		}
	}
}

function upsertRecentDocument(doc: SavedDocument) {
	recentDocuments = [doc, ...recentDocuments.filter((existing) => existing.id !== doc.id)];
	docMeta.set(doc.id, doc);
}

// Last known record per document, so an autosave can `put()` straight away.
// Re-reading the record first meant deserializing the whole document —
// including every embedded image — on each save, on the main thread.
const docMeta = new Map<string, SavedDocument>();

/** Refresh an already-listed document in place, without a new array identity. */
function touchRecentDocument(doc: SavedDocument) {
	const existing = recentDocuments.find((d) => d.id === doc.id);
	if (!existing) {
		upsertRecentDocument(doc);
		return;
	}
	existing.name = doc.name;
	existing.updatedAt = doc.updatedAt;
	docMeta.set(doc.id, doc);
}

export const documentStore = {
	get currentDocId() {
		return currentDocId;
	},
	get saveStatus() {
		return saveStatus;
	},
	get recentDocuments() {
		return recentDocuments;
	},
	get isTransitioningDocument() {
		return isTransitioningDocument;
	},

	async init(options: InitOptions = {}) {
		const { restoreCurrent = true } = options;
		if (restoreCurrent && !hasLoadedSessionCurrent && currentDocId === null) {
			hasLoadedSessionCurrent = true;
			const stored = sessionStorage.getItem('md2pdf-current-doc-id');
			if (stored) {
				currentDocId = stored;
				isTransitioningDocument = true;
			}
		}
		await this.refreshList();
	},

	async refreshList() {
		recentDocuments = await listDocuments();
		for (const doc of recentDocuments) docMeta.set(doc.id, doc);
	},

	async flushPendingSave(): Promise<void> {
		const pending = pendingSave;
		if (!pending) return;
		pendingSave = null;
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		await this.saveNow(pending.id, pending.content, pending.assets);
	},

	async loadDocument(id: string): Promise<SavedDocument | null> {
		await this.flushPendingSave();
		const doc = await getDocument(id);
		if (!doc) return null;
		docMeta.set(doc.id, doc);
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		isTransitioningDocument = true;
		setCurrentDocument(id, true);
		return doc;
	},

	async createDocument(
		content: string = '',
		assets?: Record<string, SavedDocumentAsset>,
		creationSource?: DocumentCreationSource,
	): Promise<SavedDocument> {
		await this.flushPendingSave();
		const now = Date.now();
		const doc: SavedDocument = {
			id: crypto.randomUUID(),
			name: deriveNameFromContent(content) || '',
			content,
			assets,
			creationSource,
			createdAt: now,
			updatedAt: now,
		};
		await saveDocument(doc);
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		pendingSave = null;
		isTransitioningDocument = true;
		setCurrentDocument(doc.id, true);
		upsertRecentDocument(doc);
		return doc;
	},

	setCurrentDocument(id: string | null, persistSession: boolean = true) {
		setCurrentDocument(id, persistSession);
	},

	finishDocumentTransition() {
		isTransitioningDocument = false;
	},

	async saveNow(
		id: string,
		content: string,
		assets?: Record<string, SavedDocumentAsset>,
	): Promise<void> {
		if (!id) return;
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		pendingSave = null;
		const known = docMeta.get(id) ?? (await getDocument(id));
		if (!known) {
			saveStatus = 'saved';
			return;
		}
		const next: SavedDocument = {
			...known,
			content,
			assets: assets ?? known.assets,
			name: deriveNameFromContent(content),
			updatedAt: Date.now(),
		};
		docMeta.set(id, next);
		await saveDocument(next);
		saveStatus = 'saved';
		touchRecentDocument(next);
	},

	autoSave(id: string, content: string, assets?: Record<string, SavedDocumentAsset>) {
		if (!id) return;
		if (isTransitioningDocument) return;
		saveStatus = 'saving';
		if (saveTimer) clearTimeout(saveTimer);
		// The debounce coalesces calls, so a save that carries no assets has to
		// inherit the ones a superseded call was about to write — otherwise the
		// next keystroke silently drops a freshly added image.
		const carried = assets ?? (pendingSave?.id === id ? pendingSave.assets : undefined);
		const mine = { id, content, assets: carried };
		pendingSave = mine;
		saveTimer = setTimeout(async () => {
			if (pendingSave !== mine) return;
			await this.saveNow(id, content, carried);
		}, AUTOSAVE_DEBOUNCE_MS);
	},

	async deleteDocument(id: string) {
		await this.flushPendingSave();
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		pendingSave = null;
		await deleteDocFromDB(id);
		docMeta.delete(id);
		if (currentDocId === id) {
			setCurrentDocument(null, true);
		}
		recentDocuments = recentDocuments.filter((doc) => doc.id !== id);
	},
};
