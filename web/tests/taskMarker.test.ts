import { describe, expect, it } from 'vitest';
import { taskMarker } from '../src/lib/utils/task-marker';

/** The character the editor would overwrite, for readability in failures. */
const marked = (line: string) => {
	const m = taskMarker(line);
	return m ? line.slice(0, m.at) + '<' + line[m.at] + '>' + line.slice(m.at + 1) : null;
};

describe('taskMarker', () => {
	it('finds the box behind every bullet and ordinal', () => {
		expect(marked('- [ ] a')).toBe('- [< >] a');
		expect(marked('* [ ] a')).toBe('* [< >] a');
		expect(marked('+ [x] a')).toBe('+ [<x>] a');
		expect(marked('1. [ ] a')).toBe('1. [< >] a');
		expect(marked('12) [X] a')).toBe('12) [<X>] a');
	});

	it('sees through indentation and blockquote prefixes', () => {
		expect(marked('    - [ ] nested')).toBe('    - [< >] nested');
		expect(marked('> - [x] quoted')).toBe('> - [<x>] quoted');
		expect(marked('>   - [ ] both')).toBe('>   - [< >] both');
		expect(marked('\t- [ ] tabbed')).toBe('\t- [< >] tabbed');
	});

	it('reports the state, uppercase X included', () => {
		expect(taskMarker('- [ ] a')?.checked).toBe(false);
		expect(taskMarker('- [x] a')?.checked).toBe(true);
		expect(taskMarker('- [X] a')?.checked).toBe(true);
	});

	it('refuses anything that is not a task marker', () => {
		for (const line of [
			'  [ ] no bullet',
			'- [] empty',
			'- [y] wrong letter',
			'-[ ] no space after the bullet',
			'text - [ ] mid-sentence',
			'- plain item',
			'',
			'[ ] bare',
			'--- [ ] rule',
		]) {
			expect(taskMarker(line), line).toBeNull();
		}
	});

	it('points at the character between the brackets, not the bracket', () => {
		const line = '   - [x] done';
		const m = taskMarker(line)!;
		expect(line[m.at]).toBe('x');
		expect(line[m.at - 1]).toBe('[');
		expect(line[m.at + 1]).toBe(']');
	});
});
