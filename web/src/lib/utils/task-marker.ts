/**
 * Finding the `[ ]` on a line of Markdown.
 *
 * Ticking a box in the preview edits the document, which is the one place a
 * view is allowed to write. The engine says *which line*; this says where on
 * that line the marker is, and refuses when the line does not hold one — so a
 * click against a preview that has fallen behind is a no-op rather than a
 * corruption.
 */

// Blockquote prefixes and indentation, then a bullet or an ordinal, then the
// box. Anchored, so `- [ ]` written mid-sentence is not a task marker.
const TASK_MARKER = /^[ \t>]*(?:[-*+]|\d{1,9}[.)])[ \t]+\[([ xX])\]/;

export interface TaskMarker {
	/** Offset of the character between the brackets, within the line. */
	at: number;
	checked: boolean;
}

export function taskMarker(lineText: string): TaskMarker | null {
	const match = TASK_MARKER.exec(lineText);
	if (!match) return null;
	return {
		at: match[0].length - 2,
		checked: match[1] !== ' ',
	};
}
