export interface SavedDocumentAsset {
	bytes: Uint8Array;
	mimeType: string;
}

export type DocumentCreationSource = 'template' | 'blank' | 'import';

export interface SavedDocument {
	id: string;
	name: string;
	mode: 'pdf';
	content: string;
	assets?: Record<string, SavedDocumentAsset>;
	creationSource?: DocumentCreationSource;
	createdAt: number;
	updatedAt: number;
}

const DB_NAME = 'md2pdf_documents';
const DB_VERSION = 1;
const STORE_NAME = 'documents';

// One connection for the page's lifetime. Opening a fresh IDBDatabase per
// call cost an async round-trip (and leaked a connection) on every autosave.
let dbPromise: Promise<IDBDatabase> | null = null;

function openDB(): Promise<IDBDatabase> {
	if (dbPromise) return dbPromise;
	dbPromise = new Promise<IDBDatabase>((resolve, reject) => {
		const request = indexedDB.open(DB_NAME, DB_VERSION);
		request.onerror = () => reject(request.error);
		request.onsuccess = () => {
			const db = request.result;
			// Another tab upgrading, or the connection being closed, must not
			// leave a dead handle cached.
			db.onversionchange = () => {
				db.close();
				dbPromise = null;
			};
			db.onclose = () => {
				dbPromise = null;
			};
			resolve(db);
		};
		request.onupgradeneeded = (event) => {
			const db = (event.target as IDBOpenDBRequest).result;
			if (!db.objectStoreNames.contains(STORE_NAME)) {
				db.createObjectStore(STORE_NAME, { keyPath: 'id' });
			}
		};
	});
	dbPromise.catch(() => {
		dbPromise = null;
	});
	return dbPromise;
}

export async function saveDocument(doc: SavedDocument): Promise<void> {
	const db = await openDB();
	return new Promise((resolve, reject) => {
		const tx = db.transaction(STORE_NAME, 'readwrite');
		const store = tx.objectStore(STORE_NAME);
		const request = store.put(doc);
		request.onerror = () => reject(request.error);
		request.onsuccess = () => resolve();
	});
}

export async function getDocument(id: string): Promise<SavedDocument | null> {
	const db = await openDB();
	return new Promise((resolve, reject) => {
		const tx = db.transaction(STORE_NAME, 'readonly');
		const store = tx.objectStore(STORE_NAME);
		const request = store.get(id);
		request.onerror = () => reject(request.error);
		request.onsuccess = () => resolve(request.result ?? null);
	});
}

export async function listDocuments(): Promise<SavedDocument[]> {
	const db = await openDB();
	return new Promise((resolve, reject) => {
		const tx = db.transaction(STORE_NAME, 'readonly');
		const store = tx.objectStore(STORE_NAME);
		const request = store.getAll();
		request.onerror = () => reject(request.error);
		request.onsuccess = () => {
			const docs = (request.result as SavedDocument[]) ?? [];
			docs.sort((a, b) => b.updatedAt - a.updatedAt);
			resolve(docs);
		};
	});
}

export async function deleteDocument(id: string): Promise<void> {
	const db = await openDB();
	return new Promise((resolve, reject) => {
		const tx = db.transaction(STORE_NAME, 'readwrite');
		const store = tx.objectStore(STORE_NAME);
		const request = store.delete(id);
		request.onerror = () => reject(request.error);
		request.onsuccess = () => resolve();
	});
}
