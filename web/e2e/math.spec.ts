import { expect, test } from '@playwright/test';

/**
 * Math parity between the two renderers.
 *
 * The PDF hands math to mitex; the web preview uses math-core, which accepts
 * less LaTeX and — with unknown commands ignored — reports the gap by dropping
 * an `<merror>` *inside* an otherwise-fine formula. So a document that is
 * perfect in the PDF rendered here as an error blob followed by half a
 * formula, with nothing failing anywhere to say so.
 */
test('a boxed formula renders as a box, not an error', async ({ page }) => {
	await page.goto('/');
	await page.waitForSelector('.cm-content');
	await page.locator('.cm-content').click();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.type('$$\\boxed{X\\to Y \\;\\Rightarrow\\; I(X;Y)}$$\n');

	await page.getByRole('button', { name: 'Web', exact: true }).click();

	// The document only — the shadow root also holds the engine's stylesheet,
	// which names every class it styles, `md2pdf-math-error` included.
	const rendered = () =>
		page.evaluate(
			() =>
				document.querySelector('.html-preview')?.shadowRoot?.getElementById('md2pdf-root')
					?.innerHTML ?? '',
		);

	await expect.poll(rendered).toContain('md2pdf-math-boxed');

	const html = await rendered();
	expect(html).not.toContain('math-core-unknown-cmd');
	expect(html).not.toContain('md2pdf-math-error');
	// The formula itself survived the box.
	expect(html).toContain('⇒');

	// And the browser actually draws the border we substituted for `\boxed`.
	const border = await page.evaluate(() => {
		const math = document
			.querySelector('.html-preview')
			?.shadowRoot?.querySelector('.md2pdf-math-boxed math');
		return math ? getComputedStyle(math).borderTopWidth : '';
	});
	expect(border).not.toBe('0px');
	expect(border).not.toBe('');
});
