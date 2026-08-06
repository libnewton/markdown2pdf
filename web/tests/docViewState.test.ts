import { describe, expect, it } from 'vitest';
import {
	DOC_VIEW_STATE_KEY,
	forgetDocViewState,
	pruneDocViewState,
	readDocViewState,
	updateDocViewState,
} from '$lib/storage/docViewState';

describe('per-document view state', () => {
	it('has nothing to say about a document it has never seen', () => {
		expect(readDocViewState('unknown')).toEqual({});
	});

	it('keeps each document apart', () => {
		updateDocViewState('a', { previewMode: 'pages', leftPaneWidth: 30 });
		updateDocViewState('b', { previewMode: 'document', leftPaneWidth: 70 });

		expect(readDocViewState('a')).toEqual({ previewMode: 'pages', leftPaneWidth: 30 });
		expect(readDocViewState('b')).toEqual({ previewMode: 'document', leftPaneWidth: 70 });
	});

	it('merges a partial save into what is already stored', () => {
		updateDocViewState('a', { previewMode: 'pages', leftPaneWidth: 30 });
		// `?view` saves the width alone — the mode it forces is not a choice.
		updateDocViewState('a', { leftPaneWidth: 65 });

		expect(readDocViewState('a')).toEqual({ previewMode: 'pages', leftPaneWidth: 65 });
	});

	it('ignores stored junk instead of handing it back', () => {
		localStorage.setItem(
			DOC_VIEW_STATE_KEY,
			JSON.stringify({
				a: { previewMode: 'sideways', leftPaneWidth: 'wide' },
				b: 'not an object',
			}),
		);

		expect(readDocViewState('a')).toEqual({});
		expect(readDocViewState('b')).toEqual({});
	});

	it('survives storage that is not JSON at all', () => {
		localStorage.setItem(DOC_VIEW_STATE_KEY, '{oops');
		expect(readDocViewState('a')).toEqual({});

		updateDocViewState('a', { previewMode: 'pages' });
		expect(readDocViewState('a')).toEqual({ previewMode: 'pages' });
	});

	it('forgets a deleted document without touching the others', () => {
		updateDocViewState('a', { leftPaneWidth: 30 });
		updateDocViewState('b', { leftPaneWidth: 70 });

		forgetDocViewState('a');

		expect(readDocViewState('a')).toEqual({});
		expect(readDocViewState('b')).toEqual({ leftPaneWidth: 70 });
	});

	it('prunes everything the document list no longer contains', () => {
		updateDocViewState('a', { leftPaneWidth: 30 });
		updateDocViewState('b', { leftPaneWidth: 70 });
		updateDocViewState('c', { leftPaneWidth: 40 });

		pruneDocViewState(['b']);

		expect(readDocViewState('a')).toEqual({});
		expect(readDocViewState('b')).toEqual({ leftPaneWidth: 70 });
		expect(readDocViewState('c')).toEqual({});
	});
});
