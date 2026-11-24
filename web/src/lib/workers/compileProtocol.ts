/**
 * The messages between the Typst worker and its client, shared by both sides.
 */
import type { SvgDocument } from '#lib/typst/svg-split.ts';

/**
 * Marker for a preview compile the worker dropped because a newer one arrived.
 * It travels as an error so the pending promise always settles, but it is not
 * a failure: the caller ignores it rather than showing a compile error.
 */
export const SUPERSEDED = 'md2pdf:superseded';

/** Image bytes by the path the document uses. Only new or changed ones travel. */
export type ImageMap = Record<string, Uint8Array<ArrayBuffer>>;

export type Theme = 'system' | 'light' | 'dark';

/** Run a Typst compile, for the paged preview or a PDF. */
export type CompileRequest = {
	type: 'compile';
	id: string;
	markdown: string;
	images?: ImageMap;
	format?: 'pdf' | 'preview';
};

/** Ask the engine for HTML. No Typst compile is involved. */
export type HtmlRequest = {
	type: 'html';
	id: string;
	markdown: string;
	images?: ImageMap;
	/** Wrap the fragment in a full document, for a download. */
	standalone?: boolean;
	theme?: Theme;
	/**
	 * The fragment is backed by a source the reader can edit: blocks carry
	 * their source line and task checkboxes are live. Never set for a
	 * download, which has no source to point back at.
	 */
	editable?: boolean;
};

/** Drop a preview compile that has not started yet. */
export type CancelRequest = { type: 'cancel'; id: string };

export type WorkerRequest = CompileRequest | HtmlRequest | CancelRequest;

type Result<T> = { type: 'compile-result'; id: string; ok: true; diagnostics: string[] } & T;

export type CompileResponse =
	| Result<{ pdf: ArrayBuffer }>
	| Result<{ preview: SvgDocument }>
	| Result<{ html: string }>
	| { type: 'compile-result'; id: string; ok: false; error: string; diagnostics: string[] };

/**
 * Remote images a render went ahead without have arrived (or failed for
 * good), so whatever is on screen is worth rendering again.
 */
export type RemotesReady = { type: 'remotes-ready' };

export type WorkerResponse = CompileResponse | RemotesReady;
