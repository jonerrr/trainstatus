import type { Catalog, Service } from '../src/lib/client/index.js';
import { expect, test } from './fixtures';

const dialog = (page: import('@playwright/test').Page) =>
	page.getByRole('dialog', { name: 'Transit details' });

test('opening a stop from the list can be restored with Back', async ({ page }) => {
	const stops = ((await (await page.request.get('/api/v1/catalog/mta_subway')).json()) as Catalog)
		.stops;
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
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	page.on('console', (message) => {
		if (
			message.type() === 'error' &&
			/effect_update_depth_exceeded|invariant_violation/.test(message.text())
		)
			errors.push(message.text());
	});
	const [catalog, service] = (await Promise.all([
		page.request.get('/api/v1/catalog/mta_subway').then((response) => response.json()),
		page.request.get('/api/v1/service/mta_subway').then((response) => response.json())
	])) as [Catalog, Service];
	const stop = catalog.stops[0];
	const route = catalog.routes[0];
	expect(stop).toBeTruthy();
	expect(route).toBeTruthy();
	const trip = Object.entries(service.routes)
		.map(([route_id, { trips }]) => ({ route_id, trip: trips[0] }))
		.find(({ trip }) => trip);
	await page.addInitScript(
		({ stop_id, trip }) => {
			localStorage.setItem(
				'stopPins',
				JSON.stringify({ mta_subway: [stop_id], mta_bus: [], njt_bus: [] })
			);
			localStorage.setItem(
				'tripRoutePins',
				JSON.stringify({
					mta_subway: trip ? [JSON.stringify([trip.route_id, trip.id])] : [],
					mta_bus: [],
					njt_bus: []
				})
			);
		},
		{ stop_id: stop.id, trip: trip ? { route_id: trip.route_id, id: trip.trip.id } : null }
	);

	await page.goto(`/?s=${encodeURIComponent(stop.id)}&src=mta_subway`);
	await expect(page).toHaveURL(/[?&]s=/);
	await expect(page).toHaveURL(/[?&]src=mta_subway/);
	await expect(dialog(page)).toContainText(stop.name);

	await page.goto(`/?r=${encodeURIComponent(route.id)}&src=mta_subway`);
	await expect(page).toHaveURL(/[?&]r=/);
	await expect(page).toHaveURL(/[?&]src=mta_subway/);
	await expect(dialog(page)).toContainText(route.short_name);

	if (trip) {
		const id = trip.trip.id;
		await page.goto(
			`/?t=${encodeURIComponent(id)}&r=${encodeURIComponent(trip.route_id)}&src=mta_subway`
		);
		await expect.poll(() => new URL(page.url()).searchParams.get('t')).toBe(id);
		await expect.poll(() => new URL(page.url()).searchParams.get('r')).toBe(trip.route_id);
		await expect.poll(() => new URL(page.url()).searchParams.get('src')).toBe('mta_subway');
		await expect(dialog(page)).toBeVisible();
		await expect(dialog(page).locator('summary')).toContainText('Consist');
	}

	await page.goto(`/?s=${encodeURIComponent(stop.id)}`);
	await expect(dialog(page)).toBeHidden();

	await page.goto('/?s=not-a-real-stop&src=mta_subway');
	await expect.poll(() => new URL(page.url()).searchParams.get('s')).toBeNull();
	await expect(dialog(page)).toBeHidden();
	expect(errors).toEqual([]);
});
