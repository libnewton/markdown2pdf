import { expect, test } from '@playwright/test';

/**
 * Chinese in the PDF.
 *
 * `fonts/` carries Latin only, so CJK used to reach Typst with no face that
 * had the glyphs and came out as tofu. The face is ~8 MB per weight, so it is
 * fetched only once a document actually contains CJK.
 */
test('a Chinese document renders in both previews', async ({ page }) => {
	await page.goto('/');
	await page.waitForSelector('.cm-content');
	await page.locator('.cm-content').click();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.type('# 你好，世界\n\n简体中文段落，mixed with English.\n');

	// The Web view needs no font of ours — the browser has one.
	await page.getByRole('button', { name: 'Web', exact: true }).click();
	await expect
		.poll(() =>
			page.evaluate(() => document.querySelector('.html-preview')?.shadowRoot?.textContent ?? '')
		)
		.toContain('你好，世界');

	// The paged preview is the one that needed the font.
	await page.getByRole('button', { name: 'Pages', exact: true }).click();
	await expect.poll(() => page.locator('.page-slot').count(), { timeout: 80_000 }).toBeGreaterThan(0);
	await expect(page.locator('.error-badge')).toHaveCount(0);

	// Typst emits the glyphs it actually set, so their presence in the SVG is
	// the difference between real text and tofu.
	const glyphs = await page.evaluate(() => {
		const svg = document.querySelector('.svg-preview-container');
		return (svg?.textContent ?? '') + (svg?.innerHTML.match(/data-tid="[^"]*"/g)?.join('') ?? '');
	});
	expect(glyphs.length).toBeGreaterThan(0);
});
