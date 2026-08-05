import { expect, test } from '@playwright/test';

/**
 * A document that names an image nobody can fetch still renders.
 *
 * Typst treats a missing `image()` as fatal, so one unreachable URL used to
 * take the entire paged preview down and say nothing about why. The demo
 * document itself does this on any machine without network access.
 */
test('an unreachable image degrades instead of failing the render', async ({ page }) => {
	await page.goto('/');
	await page.waitForSelector('.cm-content');
	await page.locator('.cm-content').click();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.type('# Still here\n\n![x](https://no-such-host.invalid/a.png)\n\nAfter.\n');

	// The paged preview compiles rather than erroring out.
	await expect.poll(() => page.locator('.page-slot').count(), { timeout: 80_000 }).toBeGreaterThan(0);
	await expect(page.locator('.error-badge')).toHaveCount(0);

	// And the Web view says what it could not get.
	await page.getByRole('button', { name: 'Web', exact: true }).click();
	await expect(page.locator('.warning-badge')).toBeVisible({ timeout: 30_000 });
	await page.locator('.warning-badge').click();
	await expect(page.locator('.warning-list')).toContainText('no-such-host.invalid');
});
