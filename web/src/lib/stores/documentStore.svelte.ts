import {
	saveDocument,
	getDocument,
	listDocuments,
	deleteDocument as deleteDocFromDB,
	type SavedDocument,
	type SavedDocumentAsset,
	type DocumentCreationSource
} from '$lib/storage/documents';

export type SaveStatus = 'saved' | 'saving';
type InitOptions = {
	restoreCurrent?: boolean;
};

let currentDocId = $state<string | null>(null);
let saveStatus = $state<SaveStatus>('saved');
let recentDocuments = $state<SavedDocument[]>([]);
let isTransitioningDocument = $state(false);

let saveTimer: ReturnType<typeof setTimeout> | null = null;
let hasLoadedSessionCurrent = false;
let pendingSave: { id: string; content: string; assets?: Record<string, SavedDocumentAsset> } | null = null;

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
	return (
		doc.creationSource === undefined &&
		doc.content.trim() === '' &&
		doc.name === ''
	);
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
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		isTransitioningDocument = true;
		setCurrentDocument(id, true);
		return doc;
	},

	async createDocument(
		mode: 'pdf',
		content: string = '',
		assets?: Record<string, SavedDocumentAsset>,
		creationSource?: DocumentCreationSource
	): Promise<SavedDocument> {
		await this.flushPendingSave();
		const now = Date.now();
		const doc: SavedDocument = {
			id: crypto.randomUUID(),
			name: deriveNameFromContent(content) || '',
			mode,
			content,
			assets,
			creationSource,
			createdAt: now,
			updatedAt: now
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
		assets?: Record<string, SavedDocumentAsset>
	): Promise<void> {
		if (!id) return;
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		pendingSave = null;
		const existing = await getDocument(id);
		if (!existing) {
			saveStatus = 'saved';
			return;
		}
		existing.content = content;
		existing.assets = assets;
		existing.name = deriveNameFromContent(content);
		existing.updatedAt = Date.now();
		await saveDocument(existing);
		saveStatus = 'saved';
		upsertRecentDocument({ ...existing });
	},

	autoSave(id: string, content: string, assets?: Record<string, SavedDocumentAsset>) {
		if (!id) return;
		if (isTransitioningDocument) return;
		saveStatus = 'saving';
		if (saveTimer) clearTimeout(saveTimer);
		pendingSave = { id, content, assets };
		saveTimer = setTimeout(async () => {
			const pending = pendingSave;
			if (!pending || pending.id !== id || pending.content !== content || pending.assets !== assets) {
				return;
			}
			await this.saveNow(id, content, assets);
		}, 1000);
	},

	async deleteDocument(id: string) {
		await this.flushPendingSave();
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		pendingSave = null;
		await deleteDocFromDB(id);
		if (currentDocId === id) {
			setCurrentDocument(null, true);
		}
		recentDocuments = recentDocuments.filter((doc) => doc.id !== id);
	}
};
