import type { Route, Stop, Trip } from '../src/lib/client/index.js';
import { expect, test } from './fixtures';

const dialog = (page: import('@playwright/test').Page) =>
	page.getByRole('dialog', { name: 'Transit details' });

test('opening a stop from the list can be restored with Back', async ({ page }) => {
	const stops = (await (await page.request.get('/api/v1/stops/mta_subway')).json()) as Stop[];
	const stop = stops[0];
	await page.goto('/stops');
	await page.getByRole('searchbox').fill(stop.id);
	const row = page.locator('main .list-item').filter({ hasText: stop.name });
	await expect(row).toHaveCount(1);
	await row.locator('button').first().click();

	await expect.poll(() => new URL(page.url()).searchParams.get('s')).toBe(stop.id);
	await expect.poll(() => new URL(page.url()).searchParams.get('src')).toBe('mta_subway');
	await expect(dialog(page)).toContainText(stop.name);

	await page.getByRole('button', { name: 'Close modal' }).click();
	await expect(dialog(page)).toBeHidden();
	await page.goBack();
	await expect(dialog(page)).toContainText(stop.name);
});

test('cold-load modals for subway stops, routes, and trips', async ({ page }) => {
	const [stops, routes, trips] = (await Promise.all([
		page.request.get('/api/v1/stops/mta_subway').then((response) => response.json()),
		page.request.get('/api/v1/routes/mta_subway').then((response) => response.json()),
		page.request.get('/api/v1/trips/mta_subway').then((response) => response.json())
	])) as [Stop[], Route[], Trip[]];
	const stop = stops[0];
	const route = routes[0];
	expect(stop).toBeTruthy();
	expect(route).toBeTruthy();

	await page.goto(`/?s=${encodeURIComponent(stop.id)}&src=mta_subway`);
	await expect(page).toHaveURL(/[?&]s=/);
	await expect(page).toHaveURL(/[?&]src=mta_subway/);
	await expect(dialog(page)).toContainText(stop.name);

	await page.goto(`/?r=${encodeURIComponent(route.id)}&src=mta_subway`);
	await expect(page).toHaveURL(/[?&]r=/);
	await expect(page).toHaveURL(/[?&]src=mta_subway/);
	await expect(dialog(page)).toContainText(route.short_name);

	if (trips.length === 0) {
		await page.goto('/?t=none&src=mta_subway');
		await expect(page).toHaveURL(/[?&]t=/);
		await expect(page).toHaveURL(/[?&]src=mta_subway/);
		await expect(dialog(page)).toBeHidden();
	} else {
		const trip = trips[0];
		await page.goto(`/?t=${encodeURIComponent(trip.id)}&src=mta_subway`);
		await expect.poll(() => new URL(page.url()).searchParams.get('t')).toBe(trip.id);
		await expect.poll(() => new URL(page.url()).searchParams.get('src')).toBe('mta_subway');
		await expect(dialog(page)).toBeVisible();
		await expect(dialog(page).locator('summary')).toContainText('Consist');
	}

	await page.goto(`/?s=${encodeURIComponent(stop.id)}`);
	await expect(dialog(page)).toBeHidden();

	await page.goto('/?s=not-a-real-stop&src=mta_subway');
	await expect(dialog(page)).toBeHidden();
});
