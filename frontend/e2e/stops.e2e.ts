import type { Catalog, CatalogStop } from '../src/lib/client/index.js';
import { expect, searchableStop, test } from './fixtures';

const qualifying = ['full_time', 'part_time', 'rush_hour'];

for (const mobile of [false, true]) {
	test.describe(mobile ? 'touch-first' : 'desktop', () => {
		test.use({ isMobile: mobile, hasTouch: mobile });

		test('stop search focuses only on desktop page entry', async ({ page }) => {
			await page.goto('/stops');
			const search = page.getByRole('searchbox');
			await expect(search).toBeVisible();
			if (mobile) await expect(search).not.toBeFocused();
			else await expect(search).toBeFocused();

			await page.getByRole('link', { name: 'Home', exact: true }).click();
			await page.getByRole('link', { name: 'Stops', exact: true }).click();
			await expect(search).toBeVisible();
			if (mobile) await expect(search).not.toBeFocused();
			else await expect(search).toBeFocused();
		});
	});
}

test('subway stop search filters by name, stop id, and route id', async ({ page }) => {
	const catalog = (await (await page.request.get('/api/v1/catalog/mta_subway')).json()) as Catalog;
	const stops = catalog.stops;
	const { stop: namedStop, query } = searchableStop(catalog);

	const byRoute = new Map<string, CatalogStop[]>();
	for (const stop of stops) {
		for (const route of stop.routes) {
			if (route.data.source !== 'mta_subway') continue;
			if (!qualifying.includes(route.data.stop_type)) continue;
			const list = byRoute.get(route.route_id) ?? [];
			list.push(stop);
			byRoute.set(route.route_id, list);
		}
	}
	// Exercise a long route, excluding IDs that the UI treats as exact stop searches.
	const route = [...byRoute.entries()]
		.filter(([id]) => !stops.some((stop) => stop.id === id))
		.sort((a, b) => b[1].length - a[1].length)[0];
	expect(route?.[1].length).toBeGreaterThan(0);
	const routeStops = [...route[1]].sort(
		(a, b) =>
			a.routes.find((entry) => entry.route_id === route[0])!.stop_sequence -
			b.routes.find((entry) => entry.route_id === route[0])!.stop_sequence
	);

	await page.goto('/stops');
	const items = page.locator('main .list-item');
	await expect.poll(() => items.count()).toBeGreaterThan(1);
	const viewport = page.locator('main .overflow-y-auto');
	const names = items.locator('.text-lg');
	await expect(names.first()).toHaveText(stops[0].name);

	await page.getByRole('searchbox').fill(query);
	await expect(items).toHaveCount(1);
	await expect(items.first()).toContainText(namedStop.name);

	await page.getByRole('button', { name: 'Clear search' }).click();
	await expect.poll(() => items.count()).toBeGreaterThan(1);
	await expect(names.first()).toHaveText(stops[0].name);

	const stop = stops.find(
		(candidate) =>
			!catalog.routes.some((route) => route.id === candidate.id) &&
			!candidate.name.toLowerCase().includes(candidate.id.toLowerCase())
	);
	if (!stop) throw new Error('expected a stop ID distinct from route IDs and its own name');

	await page.getByRole('searchbox').fill(stop.id);
	await expect.poll(() => items.count()).toBeGreaterThan(0);
	if ((await items.count()) > 1) await expect(names.first()).not.toHaveText(stop.name);
	await expect
		.poll(async () => {
			await viewport.evaluate((element) => {
				element.scrollTo({ top: element.scrollHeight, behavior: 'instant' });
			});
			return (await names.last().textContent())?.trim();
		})
		.toBe(stop.name);
	await items.last().locator('button').first().click();
	await expect.poll(() => new URL(page.url()).searchParams.get('s')).toBe(stop.id);
	await page.getByRole('button', { name: 'Close modal' }).click();
	await page.getByRole('button', { name: 'Clear search' }).click();
	await viewport.evaluate((element) => element.scrollTo({ top: 0, behavior: 'instant' }));
	await expect(names.first()).toHaveText(stops[0].name);

	await page.getByRole('searchbox').fill(route[0]);
	await expect(names.first()).toHaveText(routeStops[0].name);
	const expectedNames = new Set(routeStops.map((stop) => stop.name));
	for (const name of await names.allTextContents()) {
		expect(expectedNames.has(name.trim())).toBe(true);
	}

	// Only viewport rows plus overscan are mounted. Reach the final stop by scrolling.
	await expect
		.poll(async () => {
			await viewport.evaluate((element) => {
				element.scrollTo({ top: element.scrollHeight, behavior: 'instant' });
			});
			return (await names.last().textContent())?.trim();
		})
		.toBe(routeStops.at(-1)!.name);
	for (const name of await names.allTextContents()) {
		expect(expectedNames.has(name.trim())).toBe(true);
	}
	await items.last().locator('button').first().click();
	await expect.poll(() => new URL(page.url()).searchParams.get('s')).toBe(routeStops.at(-1)!.id);
});
