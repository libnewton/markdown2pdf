import { describe, expect, it } from 'vitest';
import { SLASH_COMMANDS, linkAround, slashCommand, toggleWrap } from '../src/lib/editor/commands';

describe('slashCommand', () => {
	it('matches a line that is only the command', () => {
		expect(slashCommand('/toc')?.name).toBe('toc');
		expect(slashCommand('  /new  ')?.name).toBe('new');
	});

	it('ignores a slash that is part of the prose', () => {
		for (const line of ['/toc and more', 'see /toc', 'https://x/toc', '//toc', '/', '/TOC']) {
			expect(slashCommand(line), line).toBeUndefined();
		}
	});

	it('ignores a name it does not know', () => {
		expect(slashCommand('/nope')).toBeUndefined();
	});

	it('every caret lands inside its own snippet', () => {
		for (const command of SLASH_COMMANDS) {
			expect(command.caret, command.name).toBeGreaterThanOrEqual(0);
			expect(command.caret, command.name).toBeLessThanOrEqual(command.snippet.length);
		}
	});

	it('puts the caret where you would keep typing', () => {
		const at = (name: string) => {
			const c = SLASH_COMMANDS.find((x) => x.name === name)!;
			return c.snippet.slice(0, c.caret) + '|' + c.snippet.slice(c.caret);
		};
		expect(at('code')).toBe('```|\n\n```\n');
		expect(at('math')).toBe('$$\n|\n$$\n');
		expect(at('info')).toBe(':::info\n|\n:::\n');
	});
});

describe('toggleWrap', () => {
	it('wraps a selection and reports the inner range', () => {
		expect(toggleWrap('word', '**')).toEqual({ text: '**word**', from: 2, to: 6 });
	});

	it('unwraps when the selection is already wrapped', () => {
		expect(toggleWrap('**word**', '**')).toEqual({ text: 'word', from: 0, to: 4 });
	});

	it('wraps an empty selection so typing continues inside', () => {
		expect(toggleWrap('', '_')).toEqual({ text: '__', from: 1, to: 1 });
	});

	it('does not mistake a short selection for a wrapped one', () => {
		// `**` is the markers themselves, not an empty wrapped selection.
		expect(toggleWrap('**', '**').text).toBe('******');
	});
});

describe('linkAround', () => {
	it('puts the caret in the URL', () => {
		const { text, from, to } = linkAround('label');
		expect(text).toBe('[label]()');
		expect(text.slice(from, to)).toBe('');
		expect(text[from - 1]).toBe('(');
	});
});
