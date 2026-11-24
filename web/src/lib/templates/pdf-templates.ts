import extended from '../../../../tests/extended.md?raw';

export interface Template {
	id: string;
	name: string;
	icon: string;
	content: string;
}

const date = new Date().toISOString().split('T')[0];

// The feature demo is the CLI fixture itself, with today's date stamped into
// the frontmatter, so one document is the reference for both front-ends.
const WELCOME = extended.replace(/^date: .*$/m, `date: ${date}`);

export const PDF_TEMPLATES: Template[] = [
	{ id: 'welcome', name: 'Feature Demo', icon: '🚀', content: WELCOME },
];
