import { expect, test, type Page } from '@playwright/test';

const editorText = (page: Page) =>
	page
		.locator('.cm-content')
		.textContent()
		.then((t) => t ?? '');

async function openApp(page: Page) {
	await page.goto('/');
	await page.waitForSelector('.cm-content');
	await expect.poll(() => editorText(page)).not.toBe('');
}

async function replaceDocument(page: Page, md: string) {
	await page.locator('.cm-content').click();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.type(md);
}

async function newBlank(page: Page) {
	await page.locator('.doc-name-btn').click();
	await page.getByRole('menuitem', { name: '+ New blank document' }).click();
	await expect(page.locator('.cm-content')).toHaveText('');
}

/** Wait until the autosave has landed in IndexedDB. */
async function saved(page: Page) {
	await expect(page.locator('.doc-status')).toHaveCount(0, { timeout: 10_000 });
}

test('an edit in one tab shows up in another tab on the same document', async ({ page }) => {
	await openApp(page);
	await replaceDocument(page, '# Shared\n\nfrom the first tab\n');
	await saved(page);

	const second = await page.context().newPage();
	await openApp(second);
	await expect.poll(() => editorText(second)).toContain('from the first tab');

	await page.locator('.cm-content').click();
	await page.keyboard.press('ControlOrMeta+End');
	await page.keyboard.type('\nmore from the first tab');
	await expect.poll(() => editorText(second)).toContain('more from the first tab');

	await second.locator('.cm-content').click();
	await second.keyboard.press('ControlOrMeta+End');
	await second.keyboard.type('\nreply from the second tab');
	await expect.poll(() => editorText(page)).toContain('reply from the second tab');

	// Nothing either tab typed is lost once both have saved.
	await saved(page);
	await saved(second);
	await second.close();
	await page.reload();
	await page.waitForSelector('.cm-content');
	await expect.poll(() => editorText(page)).toContain('more from the first tab');
	expect(await editorText(page)).toContain('reply from the second tab');
});

test('the document menu lists every document and closes on Escape', async ({ page }) => {
	await openApp(page);
	const trigger = page.locator('.doc-name-btn');
	for (let i = 1; i <= 11; i++) {
		await newBlank(page);
		await replaceDocument(page, `Doc ${i}`);
		await expect(trigger).toContainText(`Doc ${i}`);
	}
	await saved(page);

	await trigger.click();
	const menu = page.getByRole('menu', { name: 'Documents' });
	await expect(menu).toBeVisible();
	await expect(trigger).toHaveAttribute('aria-expanded', 'true');
	// The welcome document plus eleven: more than the ten the menu used to show.
	await expect(menu.getByRole('menuitem', { name: /^Delete / })).toHaveCount(12);
	await expect(menu.getByRole('menuitem', { name: /^Doc 11/ })).toBeFocused();

	await page.keyboard.press('ArrowDown');
	await expect(menu.getByRole('menuitem', { name: 'Delete Doc 11' })).toBeFocused();

	await page.keyboard.press('Escape');
	await expect(menu).toBeHidden();
	await expect(trigger).toBeFocused();
});

test('deleting a document asks first', async ({ page }) => {
	await openApp(page);
	await newBlank(page);
	await replaceDocument(page, 'Doomed');
	await saved(page);

	const trigger = page.locator('.doc-name-btn');
	await trigger.click();
	const menu = page.getByRole('menu', { name: 'Documents' });
	await menu.getByRole('menuitem', { name: 'Delete Doomed' }).click();
	const dialog = page.getByRole('alertdialog', { name: 'Delete this document?' });
	await expect(dialog).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'Cancel' })).toBeFocused();

	// Cancelling keeps it.
	await page.keyboard.press('Escape');
	await expect(dialog).toBeHidden();
	await expect(menu.getByRole('menuitem', { name: 'Delete Doomed' })).toHaveCount(1);

	await menu.getByRole('menuitem', { name: 'Delete Doomed' }).click();
	await dialog.getByRole('button', { name: 'Delete' }).click();
	await expect(dialog).toBeHidden();
	await expect(menu.getByRole('menuitem', { name: 'Delete Doomed' })).toHaveCount(0);
	await expect(trigger).not.toContainText('Doomed');
});

test('the About dialog takes focus, keeps it, and closes on Escape', async ({ page }) => {
	await openApp(page);
	const opener = page.getByRole('button', { name: 'About md2pdf' });
	await opener.click();
	const dialog = page.getByRole('dialog', { name: 'About md2pdf' });
	await expect(dialog).toBeVisible();
	for (let i = 0; i < 5; i++) {
		await page.keyboard.press('Tab');
		expect(await dialog.evaluate((el) => el.contains(document.activeElement))).toBe(true);
	}
	await page.keyboard.press('Shift+Tab');
	expect(await dialog.evaluate((el) => el.contains(document.activeElement))).toBe(true);
	await page.keyboard.press('Escape');
	await expect(dialog).toBeHidden();
	await expect(opener).toBeFocused();
});
