import { describe, expect, it } from 'vitest';
import { formatCompileError } from '#lib/workers/diagnostics.ts';

describe('formatCompileError', () => {
	it('strips internal file locations and call-stack hints', () => {
		expect(
			formatCompileError([
				'/md2pdf/vendor/mitex/mitex.typ:27:23: error: missing argument: den',
				'/main.typ:3:43: hint: while calling eval',
			]),
		).toBe('error: missing argument: den');
	});

	it('drops unknown-font warnings that bury the real error', () => {
		expect(
			formatCompileError([
				'/md2pdf/styles/modern-tech.typ:10:3: warning: unknown font family: libertinus sans',
				'/md2pdf/styles/modern-tech.typ:10:3: warning: unknown font family: fira code',
				'/main.typ:3:43: error: key nope does not exist in the bibliography',
			]),
		).toBe('error: key nope does not exist in the bibliography');
	});

	it('keeps one message per line, and the hints that explain it', () => {
		expect(
			formatCompileError([
				'/md2pdf/lib.typ:103:5: error: panicked with: "header-height must be a positive length"\n/md2pdf/lib.typ:103:5: hint: use pt, mm, cm, or in',
			]),
		).toBe(
			'error: panicked with: "header-height must be a positive length"\nhint: use pt, mm, cm, or in',
		);
	});

	it('leaves messages without an internal location alone', () => {
		expect(formatCompileError(['plugin errored with: error: unknown command: \\href'])).toBe(
			'plugin errored with: error: unknown command: \\href',
		);
	});
});
