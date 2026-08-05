import { expect, test, type Page } from '@playwright/test';

/** Text of the rendered document, which lives inside a shadow root. */
const documentText = (page: Page) =>
	page.evaluate(() => document.querySelector('.html-preview')?.shadowRoot?.textContent ?? '');

const editorText = (page: Page) =>
	page.evaluate(() => document.querySelector('.cm-content')?.textContent ?? '');

/**
 * Replace the whole document with `md` and wait for the preview to catch up.
 *
 * Polls on a marker from the new text rather than sleeping: the render is
 * debounced and the worker round-trips, so any fixed wait is either flaky or
 * slow.
 */
async function type(page: Page, md: string, marker?: string) {
	await page.locator('.cm-content').click();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.type(md);
	const expected = marker ?? md.split('\n').find((l) => /^[A-Za-z#-]/.test(l))?.replace(/^#+ /, '');
	if (expected) await expect.poll(() => documentText(page)).toContain(expected.slice(0, 20));
}

test.beforeEach(async ({ page }) => {
	await page.goto('/');
	await page.waitForSelector('.cm-content');
	// The welcome document has to finish loading before it can be replaced.
	await expect.poll(() => editorText(page)).not.toBe('');
	await page.getByRole('button', { name: 'Web', exact: true }).click();
});

test('a leading heading survives a frontmatter title', async ({ page }) => {
	// The bug this suite exists for: the headline was deleted from the rendered
	// document and nothing took its place.
	await type(page, '---\ntitle: From Frontmatter\n---\n\n# acc\n\nbody\n', 'body');
	const text = await documentText(page);
	expect(text).toContain('From Frontmatter');
	expect(text).toContain('acc');
});

test('ticking a checkbox writes to the document and undoes cleanly', async ({ page }) => {
	await type(page, '- [ ] first\n- [x] second\n', 'second');
	const before = await editorText(page);

	await page.evaluate(() => {
		const root = document.querySelector('.html-preview')!.shadowRoot!;
		const box = [...root.querySelectorAll<HTMLInputElement>('.md2pdf-task > input')].find(
			(b) => !b.checked
		);
		box!.click();
	});
	await expect.poll(() => editorText(page)).not.toBe(before);
	expect(await editorText(page)).toContain('[x] first');

	await page.locator('.cm-content').click();
	await page.keyboard.press('ControlOrMeta+z');
	await expect.poll(() => editorText(page)).toBe(before);
});

test('the exported document keeps its checkboxes inert', async ({ page }) => {
	await type(page, '- [ ] first\n', 'first');
	const live = await page.evaluate(() => {
		const root = document.querySelector('.html-preview')!.shadowRoot!;
		return root.querySelector<HTMLInputElement>('.md2pdf-task > input')!.disabled;
	});
	expect(live).toBe(false);

	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export' }).click();
	await page.getByRole('button', { name: /HTML/ }).click();
	const file = await (await download).createReadStream();
	const html = await new Promise<string>((resolve) => {
		let out = '';
		file.on('data', (c) => (out += c));
		file.on('end', () => resolve(out));
	});
	expect(html).toContain('type="checkbox" disabled');
	expect(html).not.toContain('data-md-line');
});

test('outline navigation reaches the URL', async ({ page }) => {
	// Two headings *in the body*: a leading H1 with no frontmatter title
	// becomes the title and leaves, and the drawer needs two to appear.
	await type(page, '## One\n\na\n\n## Two\n\nb\n', 'Two');
	await page.evaluate(() => {
		const root = document.querySelector('.html-preview')!.shadowRoot!;
		(root.getElementById('md2pdf-toc-state') as HTMLInputElement).checked = true;
		root.querySelectorAll<HTMLAnchorElement>('.md2pdf-toc a')[1].click();
	});
	await expect.poll(() => page.url()).toContain('#two');
});

test('view mode writes ?view without an empty value', async ({ page }) => {
	await page.getByRole('button', { name: 'Document only' }).click();
	await expect.poll(() => page.url()).toContain('?view');
	expect(page.url()).not.toContain('=#');
	expect(page.url()).not.toContain('view=');
});

test('the shortcut overlay opens on ? and closes on Escape', async ({ page }) => {
	await page.locator('.preview-pane').click();
	await page.keyboard.press('?');
	await expect(page.locator('.shortcuts')).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(page.locator('.shortcuts')).toBeHidden();
});

test('a slash command expands on Enter', async ({ page }) => {
	await type(page, '/toc', '');
	await page.keyboard.press('Enter');
	await expect.poll(() => editorText(page)).toContain('[toc]');
});

test('a diagram is embedded as an image, never as inline markup', async ({ page }) => {
	await type(page, '```mermaid\ngraph LR\n  A["</svg><script>x</script>"] --> B\n```\n', '');
	await page.waitForTimeout(2000);
	await expect
		.poll(async () =>
			page.evaluate(() => {
				const fig = document.querySelector('.html-preview')!.shadowRoot!.querySelector('.md2pdf-mermaid');
				return { img: !!fig?.querySelector('img'), svg: !!fig?.querySelector('svg') };
			})
		)
		.toEqual({ img: true, svg: false });
});

test('no document script is ever hoisted into the page', async ({ page }) => {
	await type(page, '```js\nconst a = 1\n```\n', 'const a');
	const hoisted = await page.evaluate(
		() => [...document.head.querySelectorAll('script')].filter((s) => s.textContent?.includes('md2pdfBound')).length
	);
	expect(hoisted).toBe(0);
});

test('the paged preview compiles', async ({ page }) => {
	await type(page, '# Title\n\nSome text.\n', 'Some text');
	await page.getByRole('button', { name: 'Pages', exact: true }).click();
	await expect.poll(() => page.locator('.page-slot').count(), { timeout: 80_000 }).toBeGreaterThan(0);
});
