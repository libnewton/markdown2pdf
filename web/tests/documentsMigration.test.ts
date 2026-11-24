import 'fake-indexeddb/auto';
import { IDBFactory } from 'fake-indexeddb';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const image = { bytes: new Uint8Array([137, 80, 78, 71]), mimeType: 'image/png' };

function request<T>(req: IDBRequest<T>): Promise<T> {
	return new Promise((resolve, reject) => {
		req.onsuccess = () => resolve(req.result);
		req.onerror = () => reject(req.error);
	});
}

/** A database exactly as v1 of the app left it: images inside each record. */
async function seedVersion1() {
	const open = indexedDB.open('md2pdf_documents', 1);
	open.onupgradeneeded = () => open.result.createObjectStore('documents', { keyPath: 'id' });
	const db = await request(open);
	const tx = db.transaction('documents', 'readwrite');
	const store = tx.objectStore('documents');
	store.put({
		id: 'with-image',
		name: 'Pictures',
		content: '![a](images/a.png)',
		assets: { 'images/a.png': image },
		creationSource: 'blank',
		createdAt: 1,
		updatedAt: 2,
	});
	store.put({ id: 'plain', name: 'Plain', content: '# Plain', createdAt: 1, updatedAt: 3 });
	await new Promise((resolve) => (tx.oncomplete = resolve));
	db.close();
}

async function rawRecord(store: string, key: string) {
	const db = await request(indexedDB.open('md2pdf_documents'));
	const value = await request(db.transaction(store).objectStore(store).get(key));
	db.close();
	return value;
}

beforeEach(() => {
	globalThis.indexedDB = new IDBFactory();
	vi.resetModules();
});

describe('IndexedDB v1 → v2', () => {
	it('moves each document’s images into the assets store', async () => {
		await seedVersion1();
		const storage = await import('#lib/storage/documents.ts');

		const list = await storage.listDocuments();
		expect(list.map((d) => d.id)).toEqual(['plain', 'with-image']);
		expect(list.every((d) => !('assets' in d))).toBe(true);

		const doc = await storage.getDocument('with-image');
		expect(doc?.content).toBe('![a](images/a.png)');
		expect(doc?.assets?.['images/a.png'].bytes).toEqual(image.bytes);
		expect(await storage.getDocument('plain')).not.toHaveProperty('assets');
	});

	it('leaves no images in the document records', async () => {
		await seedVersion1();
		const storage = await import('#lib/storage/documents.ts');
		await storage.listDocuments();

		expect(await rawRecord('documents', 'with-image')).not.toHaveProperty('assets');
		expect(await rawRecord('assets', 'with-image')).toEqual({ 'images/a.png': image });
		expect(await rawRecord('assets', 'plain')).toBeUndefined();
	});

	it('writes images only when a save carries them, and deletes them with the document', async () => {
		const storage = await import('#lib/storage/documents.ts');
		const record = { id: 'd', name: 'D', content: 'x', createdAt: 1, updatedAt: 1 };
		await storage.saveDocument(record, { 'images/a.png': image });
		await storage.saveDocument({ ...record, content: 'y', updatedAt: 2 });

		const doc = await storage.getDocument('d');
		expect(doc?.content).toBe('y');
		expect(Object.keys(doc?.assets ?? {})).toEqual(['images/a.png']);

		await storage.deleteDocument('d');
		expect(await storage.getDocument('d')).toBeNull();
		expect(await rawRecord('assets', 'd')).toBeUndefined();
	});
});
