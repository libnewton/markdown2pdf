import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, it } from 'vitest';
import { PDF_TEMPLATES } from '../src/lib/templates/pdf-templates';

const repo = (p: string) => fileURLToPath(new URL('../../' + p, import.meta.url));

/**
 * The feature demo exists twice: as the CLI fixture and as the app's welcome
 * document. The README says they must match, and until now nothing checked —
 * so a change to one silently left the other describing a different product.
 */
it('the welcome document matches tests/extended.md', () => {
	const fixture = readFileSync(repo('tests/extended.md'), 'utf8');
	const welcome = PDF_TEMPLATES.find((t) => t.id === 'welcome')?.content ?? '';
	// The template stamps today's date in; that one line is allowed to differ.
	const undated = (s: string) => s.replace(/^date: .*$/m, 'date: <stamped>').trimEnd();
	expect(undated(welcome)).toBe(undated(fixture));
});
