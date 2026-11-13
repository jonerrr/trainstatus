import type { Catalog } from '../src/lib/client/index.js';
import { expect, searchableStop, test } from './fixtures';

test('a pinned stop shows on the home page until it is unpinned', async ({ page }) => {
	const catalog = (await (await page.request.get('/api/v1/catalog/mta_subway')).json()) as Catalog;
	const { stop, query } = searchableStop(catalog, ['1']);

	await page.goto('/stops');
	await page.getByRole('searchbox').fill(query);
	const row = page.locator('main .list-item').filter({ hasText: stop.name });
	await expect(row).toHaveCount(1);
	await row.getByRole('button', { name: 'Pin to home screen' }).click();

	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Pinned stops' })).toBeVisible();
	const pinnedRow = page.locator('main .list-item').filter({ hasText: stop.name });
	await expect(pinnedRow).toBeVisible();
	await pinnedRow.getByRole('button', { name: 'Pin to home screen' }).click();
	await expect(pinnedRow).toHaveCount(0);
});
