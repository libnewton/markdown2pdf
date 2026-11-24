import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
	await page.goto('/');
	await page.waitForSelector('.cm-content');
});

test('the editor stays dark in both themes', async ({ page }) => {
	const background = () =>
		page.locator('.cm-editor').evaluate((el) => getComputedStyle(el).backgroundColor);
	const theme = () => page.evaluate(() => document.documentElement.dataset.theme);

	if ((await theme()) !== 'light')
		await page.getByRole('button', { name: 'Switch to light' }).click();
	const light = await background();
	await page.getByRole('button', { name: 'Switch to dark' }).click();
	expect(await background()).toBe(light);
	expect(light).toBe('rgb(40, 44, 52)');
});

test('the logo keeps its size on a phone', async ({ page }) => {
	await page.setViewportSize({ width: 375, height: 812 });
	const box = await page.locator('.logo-link').boundingBox();
	expect(box!.width).toBeGreaterThan(40);
	expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(375);
});

test('a narrow preview pane wraps its toolbar instead of clipping it', async ({ page }) => {
	await page.getByRole('button', { name: 'Pages', exact: true }).click();
	await page.getByRole('separator', { name: 'Editor width' }).focus();
	await page.keyboard.press('End');
	const pane = (await page.locator('.preview-pane').boundingBox())!;
	for (const name of ['Zoom out', 'Zoom in', 'Fit to width']) {
		const button = (await page.getByRole('button', { name, exact: true }).boundingBox())!;
		expect(button.x + button.width).toBeLessThanOrEqual(pane.x + pane.width + 0.5);
		expect(button.y + button.height).toBeLessThanOrEqual(pane.y + pane.height);
	}
});
