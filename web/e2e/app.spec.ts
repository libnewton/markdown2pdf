import { expect, test, type Page } from '@playwright/test';

/** Text of the rendered document, which lives inside a shadow root. */
const documentText = (page: Page) =>
	page
		.locator('.html-preview .md2pdf-root')
		.textContent()
		.then((text) => text ?? '');

const editorText = (page: Page) =>
	page
		.locator('.cm-content')
		.textContent()
		.then((text) => text ?? '');

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
	const expected =
		marker ??
		md
			.split('\n')
			.find((l) => /^[A-Za-z#-]/.test(l))
			?.replace(/^#+ /, '');
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
	await type(page, '---\ntitle: From Frontmatter\n---\n\n# acc\n\nbody\n', 'From Frontmatter');
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
			(b) => !b.checked,
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

test('the divider restores mouse and keyboard widths after reload', async ({ page }) => {
	const divider = page.getByRole('separator', { name: 'Editor width' });
	const box = (await divider.boundingBox())!;
	const viewport = page.viewportSize()!;
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(viewport.width * 0.64, box.y + box.height / 2);
	await page.mouse.up();
	await expect
		.poll(() => page.evaluate(() => localStorage.getItem('md2pdf-pane-width')))
		.not.toBeNull();
	await page.reload();
	await expect(page.getByRole('separator', { name: 'Editor width' })).toHaveAttribute(
		'aria-valuenow',
		/6[0-8]/,
	);

	await page.getByRole('separator', { name: 'Editor width' }).press('End');
	await expect
		.poll(() => page.evaluate(() => localStorage.getItem('md2pdf-pane-width')))
		.toBe('80');
	await page.reload();
	await expect(page.getByRole('separator', { name: 'Editor width' })).toHaveAttribute(
		'aria-valuenow',
		'80',
	);
});

test('a malformed stored divider width falls back to half', async ({ page }) => {
	await page.evaluate(() => localStorage.setItem('md2pdf-pane-width', 'not-a-number'));
	await page.reload();
	await expect(page.getByRole('separator', { name: 'Editor width' })).toHaveAttribute(
		'aria-valuenow',
		'50',
	);
});

test('the selected preview mode survives reloads', async ({ page }) => {
	await page.getByRole('button', { name: 'Pages', exact: true }).click();
	await expect
		.poll(() => page.evaluate(() => localStorage.getItem('md2pdf-preview-mode')))
		.toBe('pages');
	await page.reload();
	await expect(page.getByRole('button', { name: 'Pages', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true',
	);

	await page.getByRole('button', { name: 'Web', exact: true }).click();
	await expect
		.poll(() => page.evaluate(() => localStorage.getItem('md2pdf-preview-mode')))
		.toBe('web');
	await page.reload();
	await expect(page.getByRole('button', { name: 'Web', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true',
	);
});

test('an invalid stored preview mode falls back to Pages', async ({ page }) => {
	await page.evaluate(() => localStorage.setItem('md2pdf-preview-mode', 'invalid'));
	await page.reload();
	await expect(page.getByRole('button', { name: 'Pages', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true',
	);
});

test('Web warnings are scoped to the Web preview', async ({ page }) => {
	await type(page, '$\\rule{1em}{1pt}$\n', '');
	await expect(page.locator('.warning-badge')).toBeVisible();
	await page.getByRole('button', { name: 'Pages', exact: true }).click();
	await expect(page.locator('.warning-badge')).toBeHidden();
	await page.getByRole('button', { name: 'Web', exact: true }).click();
	await expect(page.locator('.warning-badge')).toBeVisible();
});

test('plain Web documents have no formula warning', async ({ page }) => {
	await type(page, 'No formulas here.\nStill no formulas.\n', 'No formulas');
	await expect(page.locator('.warning-badge')).toBeHidden();
});

test('HTML export captures the selected theme and owns the only theme toggle', async ({ page }) => {
	await type(page, '## Theme export\n', 'Theme export');
	const current = await page.evaluate(() => document.documentElement.dataset.theme);
	if (current !== 'dark') await page.getByRole('button', { name: 'Switch to dark' }).click();
	const previewToggle = await page.evaluate(
		() =>
			document.querySelector('.html-preview')?.shadowRoot?.querySelectorAll('.md2pdf-theme-toggle')
				.length ?? -1,
	);
	expect(previewToggle).toBe(0);

	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export' }).click();
	await page.getByRole('button', { name: /HTML/ }).click();
	const stream = await (await download).createReadStream();
	const html = await new Promise<string>((resolve) => {
		let out = '';
		stream.on('data', (chunk) => (out += chunk));
		stream.on('end', () => resolve(out));
	});
	expect(html).toContain('<html lang="en" data-theme="dark">');
	expect(html).toContain('class="md2pdf-theme-toggle"');
	expect(html).toContain('.md2pdf-theme-moon, .md2pdf-theme-sun {\n  display: block;');
	expect(html.indexOf('class="md2pdf-theme-toggle"')).toBeGreaterThan(html.indexOf('</main>'));
	expect(html).toContain(
		'@media (max-width: 640px) {\n  .md2pdf-theme-toggle {\n    position: static;',
	);
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

	// `toBeVisible` alone passed while the overlay rendered as an unstyled
	// block at the bottom of the page: its styles lived in another component's
	// scoped block and never reached it. So assert it is actually a modal.
	const backdrop = page.locator('.modal-backdrop');
	await expect(backdrop).toHaveCSS('position', 'fixed');

	const viewport = page.viewportSize()!;
	const box = (await page.locator('.shortcuts').boundingBox())!;
	expect(box.height).toBeLessThanOrEqual(viewport.height);
	expect(box.y).toBeGreaterThanOrEqual(0);
	expect(Math.abs(box.x + box.width / 2 - viewport.width / 2)).toBeLessThan(4);

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
				const fig = document
					.querySelector('.html-preview')!
					.shadowRoot!.querySelector('.md2pdf-mermaid');
				return { img: !!fig?.querySelector('img'), svg: !!fig?.querySelector('svg') };
			}),
		)
		.toEqual({ img: true, svg: false });
});

test('no document script is ever hoisted into the page', async ({ page }) => {
	await type(page, '```js\nconst a = 1\n```\n', 'const a');
	const hoisted = await page.evaluate(
		() =>
			[...document.head.querySelectorAll('script')].filter((s) =>
				s.textContent?.includes('md2pdfBound'),
			).length,
	);
	expect(hoisted).toBe(0);
});

test('the paged preview compiles', async ({ page }) => {
	await type(page, '# Title\n\nSome text.\n', 'Some text');
	await page.getByRole('button', { name: 'Pages', exact: true }).click();
	await expect
		.poll(() => page.locator('.page-slot').count(), { timeout: 80_000 })
		.toBeGreaterThan(0);
});
