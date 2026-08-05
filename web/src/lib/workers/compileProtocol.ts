/**
 * Shared between the Typst worker and its client.
 *
 * Marker for a preview compile the worker dropped because a newer one arrived.
 * It travels as an error so the pending promise always settles, but it is not
 * a failure: the caller ignores it rather than showing a compile error.
 */
export const SUPERSEDED = 'md2pdf:superseded';

/** Ask the engine for HTML. No Typst compile is involved. */
export type HtmlRequest = {
	type: 'html';
	id: string;
	markdown: string;
	images?: Record<string, Uint8Array<ArrayBuffer>>;
	/** Wrap the fragment in a full document, for a download. */
	standalone?: boolean;
	/**
	 * The fragment is backed by a source the reader can edit: blocks carry
	 * their source line and task checkboxes are live. Never set for a
	 * download, which has no source to point back at.
	 */
	editable?: boolean;
};
