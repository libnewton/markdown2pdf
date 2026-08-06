import { expect, test, type Page } from '@playwright/test';

const editorText = (page: Page) =>
	page.evaluate(() => document.querySelector('.cm-content')?.textContent ?? '');

/** The welcome document has to finish loading before it owns any layout. */
async function open(page: Page) {
	await page.goto('/');
	await page.waitForSelector('.cm-content');
	await expect.poll(() => editorText(page)).not.toBe('');
}

const editorWidth = (page: Page) =>
	page.evaluate(
		() => document.querySelector<HTMLElement>('.editor-pane')!.getBoundingClientRect().width,
	);

test.beforeEach(async ({ page }) => {
	await open(page);
});

test('a document opens in the Web preview until it is told otherwise', async ({ page }) => {
	await expect(page.getByRole('button', { name: 'Web', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true',
	);

	await page.getByRole('button', { name: 'Pages', exact: true }).click();
	await open(page);

	// The choice belongs to the document, so it survives the reload.
	await expect(page.getByRole('button', { name: 'Pages', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true',
	);
});

test('the divider stays where it was dragged', async ({ page }) => {
	const before = await editorWidth(page);
	const resizer = page.locator('.resizer');
	const box = (await resizer.boundingBox())!;

	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(box.x - 200, box.y + box.height / 2, { steps: 8 });
	await page.mouse.up();

	const dragged = await editorWidth(page);
	expect(dragged).toBeLessThan(before - 100);

	await open(page);
	expect(Math.abs((await editorWidth(page)) - dragged)).toBeLessThan(4);
});

test('a second document keeps its own layout', async ({ page }) => {
	await page.getByRole('button', { name: 'Pages', exact: true }).click();

	// A new document starts from the default again, not from the last one's.
	await page.keyboard.press('ControlOrMeta+Alt+n');
	await expect.poll(() => editorText(page)).toBe('');
	await expect(page.getByRole('button', { name: 'Web', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true',
	);

	// And the first one still remembers Pages when it is opened again.
	await page.locator('.doc-name-btn').click();
	await page.locator('.doc-item').nth(1).click();
	await expect.poll(() => editorText(page)).not.toBe('');
	await expect(page.getByRole('button', { name: 'Pages', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true',
	);
});
