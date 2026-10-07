import type { Stop } from '../src/lib/client/index.js';
import { expect, test } from './fixtures';

test('a pinned stop shows on the home page until it is unpinned', async ({ page }) => {
	const stops = (await (await page.request.get('/api/v1/stops/mta_subway')).json()) as Stop[];
	// Stop id "1" is the default subway pin, so pinning it would toggle it off.
	const stop = stops.find((candidate) => candidate.id !== '1');
	if (!stop) throw new Error('expected a subway stop other than the default pin');

	await page.goto('/stops');
	await page.getByRole('searchbox').fill(stop.id);
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
