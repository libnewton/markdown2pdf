/**
 * Slash commands and the inline formatting shortcuts, kept out of the editor
 * component so the expansion rules are testable without a DOM.
 */

export interface SlashCommand {
	name: string;
	label: string;
	/** What replaces the `/name` line. */
	snippet: string;
	/**
	 * Where the caret lands, as an offset into `snippet`. An explicit number
	 * rather than a marker character, because `$$` is a snippet of its own and
	 * any marker worth having is a character some snippet needs.
	 */
	caret: number;
}

export const SLASH_COMMANDS: SlashCommand[] = [
	{ name: 'new', label: 'New document', snippet: '', caret: 0 },
	{ name: 'toc', label: 'Table of contents', snippet: '[toc]\n\n', caret: 7 },
	{ name: 'pagebreak', label: 'Page break', snippet: '[[pagebreak]]\n\n', caret: 15 },
	{
		name: 'table',
		label: 'Table',
		snippet: '| Column | Column |\n| --- | --- |\n| Cell | Cell |\n',
		caret: 2,
	},
	{ name: 'code', label: 'Code block', snippet: '```\n\n```\n', caret: 3 },
	{ name: 'info', label: 'Callout', snippet: ':::info\n\n:::\n', caret: 8 },
	{ name: 'math', label: 'Math block', snippet: '$$\n\n$$\n', caret: 3 },
];

/**
 * The command a line names, if the whole line is exactly `/name`. Anchored to
 * the line so a URL path or an italic marker mid-sentence is never one.
 */
export function slashCommand(line: string): SlashCommand | undefined {
	const match = /^\/([a-z]+)$/.exec(line.trim());
	return match ? SLASH_COMMANDS.find((c) => c.name === match[1]) : undefined;
}

/**
 * Wrap `selected` in `marker`, or unwrap it when it is already wrapped, and
 * report where the selection should end up.
 */
export function toggleWrap(
	selected: string,
	marker: string,
): { text: string; from: number; to: number } {
	const wrapped =
		selected.length >= marker.length * 2 &&
		selected.startsWith(marker) &&
		selected.endsWith(marker);
	if (wrapped) {
		const inner = selected.slice(marker.length, -marker.length);
		return { text: inner, from: 0, to: inner.length };
	}
	return {
		text: marker + selected + marker,
		from: marker.length,
		to: marker.length + selected.length,
	};
}

/** `[selected]($)`, with the caret in the URL where you are about to type. */
export function linkAround(selected: string): { text: string; from: number; to: number } {
	const text = `[${selected}]()`;
	return { text, from: text.length - 1, to: text.length - 1 };
}
