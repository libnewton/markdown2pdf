import { expect, test } from '@playwright/test';

test('once installed, the app compiles with no network at all', async ({ page, context }) => {
	test.setTimeout(180_000);
	await page.goto('/');
	await page.waitForSelector('.cm-content');
	// The service worker precaches the compilers, the package and the fonts on
	// install, then takes control of the page.
	await page.evaluate(() => navigator.serviceWorker.ready);
	await expect
		.poll(() => page.evaluate(() => !!navigator.serviceWorker.controller), { timeout: 120_000 })
		.toBe(true);

	await context.setOffline(true);
	await page.reload();
	await page.waitForSelector('.cm-content');
	await page.locator('.cm-content').click();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.type('# Offline\n\nInline math $x^2$ and code `y`.\n');

	await page.getByRole('button', { name: 'Pages', exact: true }).click();
	await expect
		.poll(() => page.locator('.page-slot').count(), { timeout: 80_000 })
		.toBeGreaterThan(0);
	await expect(page.locator('.error-badge')).toHaveCount(0);

	await page.getByRole('button', { name: 'Web', exact: true }).click();
	await expect
		.poll(() =>
			page
				.locator('.html-preview #md2pdf-root')
				.textContent()
				.then((t) => t ?? ''),
		)
		.toContain('Offline');
});
