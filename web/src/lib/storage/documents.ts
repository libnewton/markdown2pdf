export interface SavedDocumentAsset {
	bytes: Uint8Array;
	mimeType: string;
}

export type DocumentAssets = Record<string, SavedDocumentAsset>;

export type DocumentCreationSource = 'template' | 'blank' | 'import';

/** A document without its images: what the document list holds. */
export interface DocumentRecord {
	id: string;
	name: string;
	content: string;
	creationSource?: DocumentCreationSource;
	createdAt: number;
	updatedAt: number;
}

export interface SavedDocument extends DocumentRecord {
	assets?: DocumentAssets;
}

const DB_NAME = 'md2pdf_documents';
// v1 kept each document's images inside its record, so listing the documents
// deserialised every image of every document. v2 keeps them apart.
const DB_VERSION = 2;
const DOCUMENTS = 'documents';
// One entry per document that has images, keyed by the document's id.
const ASSETS = 'assets';

// One connection for the page's lifetime. Opening a fresh IDBDatabase per
// call cost an async round-trip (and leaked a connection) on every autosave.
let dbPromise: Promise<IDBDatabase> | null = null;

/** Move v1's embedded images into the assets store. */
function splitAssets(tx: IDBTransaction): void {
	const documents = tx.objectStore(DOCUMENTS);
	const assets = tx.objectStore(ASSETS);
	documents.openCursor().onsuccess = (event) => {
		const cursor = (event.target as IDBRequest<IDBCursorWithValue | null>).result;
		if (!cursor) return;
		const { assets: images, ...record } = cursor.value as SavedDocument;
		if (images && Object.keys(images).length > 0) assets.put(images, record.id);
		cursor.update(record);
		cursor.continue();
	};
}

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
			const db = request.result;
			if (!db.objectStoreNames.contains(DOCUMENTS)) {
				db.createObjectStore(DOCUMENTS, { keyPath: 'id' });
			}
			if (!db.objectStoreNames.contains(ASSETS)) db.createObjectStore(ASSETS);
			if (event.oldVersion === 1) splitAssets(request.transaction!);
		};
	});
	dbPromise.catch(() => {
		dbPromise = null;
	});
	return dbPromise;
}

function done(tx: IDBTransaction): Promise<void> {
	return new Promise((resolve, reject) => {
		tx.oncomplete = () => resolve();
		tx.onerror = () => reject(tx.error);
		tx.onabort = () => reject(tx.error);
	});
}

function result<T>(request: IDBRequest<T>): Promise<T> {
	return new Promise((resolve, reject) => {
		request.onsuccess = () => resolve(request.result);
		request.onerror = () => reject(request.error);
	});
}

/**
 * Write the document. Its images are written only when `assets` is given, so
 * ordinary typing never re-serialises them.
 */
export async function saveDocument(doc: DocumentRecord, assets?: DocumentAssets): Promise<void> {
	const db = await openDB();
	const tx = db.transaction([DOCUMENTS, ASSETS], 'readwrite');
	const { assets: _embedded, ...record } = doc as SavedDocument;
	tx.objectStore(DOCUMENTS).put(record);
	if (assets) {
		if (Object.keys(assets).length > 0) tx.objectStore(ASSETS).put(assets, doc.id);
		else tx.objectStore(ASSETS).delete(doc.id);
	}
	return done(tx);
}

/** One document, images included. */
export async function getDocument(id: string): Promise<SavedDocument | null> {
	const db = await openDB();
	const tx = db.transaction([DOCUMENTS, ASSETS], 'readonly');
	const [record, assets] = await Promise.all([
		result<DocumentRecord | undefined>(tx.objectStore(DOCUMENTS).get(id)),
		result<DocumentAssets | undefined>(tx.objectStore(ASSETS).get(id)),
	]);
	if (!record) return null;
	return assets ? { ...record, assets } : record;
}

/** Every document, newest first, without images. */
export async function listDocuments(): Promise<DocumentRecord[]> {
	const db = await openDB();
	const tx = db.transaction(DOCUMENTS, 'readonly');
	const docs = (await result(tx.objectStore(DOCUMENTS).getAll())) as DocumentRecord[];
	return docs.sort((a, b) => b.updatedAt - a.updatedAt);
}

export async function deleteDocument(id: string): Promise<void> {
	const db = await openDB();
	const tx = db.transaction([DOCUMENTS, ASSETS], 'readwrite');
	tx.objectStore(DOCUMENTS).delete(id);
	tx.objectStore(ASSETS).delete(id);
	return done(tx);
}
