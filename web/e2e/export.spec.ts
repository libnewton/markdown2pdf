import { expect, test, type Page } from '@playwright/test';

/** A 1×1 red PNG. */
const PNG = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==',
	'base64',
);

// A citation key the bibliography does not have: a hard Typst error.

// A corrupt image is one of the few inputs Typst cannot degrade gracefully:
// decoding it fails the whole compile.
async function breakDocument(page: Page) {
	await replaceDocument(page, '# Broken\n\n');
	await page.locator('input[type="file"]').setInputFiles({
		name: 'broken.png',
		mimeType: 'image/png',
		buffer: Buffer.from('not a png'),
	});
	await expect(page.locator('.cm-content')).toContainText('![broken](images/');
}

async function replaceDocument(page: Page, md: string) {
	await page.locator('.cm-content').click();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.type(md);
}

test.beforeEach(async ({ page }) => {
	await page.goto('/');
	await page.waitForSelector('.cm-content');
	await expect.poll(() => page.locator('.cm-content').textContent()).not.toBe('');
});

test('an image added on the Web tab reaches the paged preview', async ({ page }) => {
	await page.getByRole('button', { name: 'Web', exact: true }).click();
	await replaceDocument(page, '# With image\n\n');
	await page.locator('input[type="file"]').setInputFiles({
		name: 'dot.png',
		mimeType: 'image/png',
		buffer: PNG,
	});
	await expect(page.locator('.cm-content')).toContainText('![dot](images/');
	// The Web view has the image, so the worker has seen its bytes once.
	await expect
		.poll(() =>
			page.evaluate(
				() => document.querySelector('.html-preview')!.shadowRoot!.querySelectorAll('img').length,
			),
		)
		.toBeGreaterThan(0);

	await page.getByRole('button', { name: 'Pages', exact: true }).click();
	await expect
		.poll(() => page.locator('.page-slot').count(), { timeout: 80_000 })
		.toBeGreaterThan(0);
	await expect(page.locator('.error-badge')).toHaveCount(0);
	await expect(page.locator('.error-bar')).toHaveCount(0);
});

test('PDF export downloads a file named after the document', async ({ page }) => {
	await replaceDocument(page, '# Quarterly Report\n\nBody.\n');
	await expect
		.poll(() => page.locator('.page-slot').count(), { timeout: 80_000 })
		.toBeGreaterThan(0);
	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	await page.getByRole('button', { name: 'PDF', exact: true }).click();
	expect((await download).suggestedFilename()).toBe('Quarterly Report.pdf');
});

test('a failed PDF export says so and opens nothing', async ({ page }) => {
	const popups: Page[] = [];
	page.on('popup', (popup) => popups.push(popup));
	const pageErrors: Error[] = [];
	page.on('pageerror', (error) => pageErrors.push(error));

	// Let one good compile land, so the PDF item is enabled.
	await replaceDocument(page, '# Fine\n');
	await expect
		.poll(() => page.locator('.page-slot').count(), { timeout: 80_000 })
		.toBeGreaterThan(0);
	await page.getByRole('button', { name: 'Web', exact: true }).click();
	await breakDocument(page);

	await page.keyboard.press('ControlOrMeta+p');
	await expect(page.locator('.error-bar')).toContainText('Export failed', { timeout: 60_000 });
	expect(popups).toHaveLength(0);
	expect(pageErrors).toHaveLength(0);
});

test('a compile error reads as a message, not a stack of internal paths', async ({ page }) => {
	await breakDocument(page);
	const bar = page.locator('.error-bar');
	await expect(bar).toContainText('decode image', { timeout: 80_000 });
	const text = (await bar.textContent()) ?? '';
	expect(text).not.toContain('/md2pdf/');
	expect(text).not.toContain('main.typ');
	expect(text).not.toContain('unknown font family');
	expect(await bar.evaluate((el) => getComputedStyle(el).whiteSpace)).toBe('pre-wrap');
});

test('a local image nobody added gets a placeholder, in the pages and the PDF', async ({
	page,
}) => {
	await replaceDocument(page, '# Missing picture\n\n![gone](images/never-added.png)\n\nAfter.\n');
	await expect
		.poll(() => page.locator('.page-slot').count(), { timeout: 80_000 })
		.toBeGreaterThan(0);
	await expect(page.locator('.cm-content')).toContainText('After.');
	await expect(page.locator('.error-badge')).toHaveCount(0);
	await expect(page.locator('.error-bar')).toHaveCount(0);

	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	await page.getByRole('button', { name: 'PDF', exact: true }).click();
	expect((await download).suggestedFilename()).toBe('Missing picture.pdf');
	await expect(page.locator('.error-bar')).toHaveCount(0);
});
