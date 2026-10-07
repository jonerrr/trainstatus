import type { Stop } from '../src/lib/client/index.js';
import { expect, test } from './fixtures';

const qualifying = ['full_time', 'part_time', 'rush_hour'];

test('subway stop search filters by name, stop id, and route id', async ({ page }) => {
	const stops = (await (await page.request.get('/api/v1/stops/mta_subway')).json()) as Stop[];
	const counts = new Map<string, number>();
	for (const stop of stops) {
		for (const word of stop.name.toLowerCase().split(/[^a-z0-9]+/)) {
			if (word.length < 5) continue;
			counts.set(word, (counts.get(word) ?? 0) + 1);
		}
	}
	const word = [...counts.entries()].find(([, count]) => count === 1)?.[0];
	expect(word).toBeTruthy();
	const named = stops.filter((stop) => stop.name.toLowerCase().includes(word!));

	const byRoute = new Map<string, Stop[]>();
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
	const restored = await items.count();

	await page.getByRole('searchbox').fill(word!);
	await expect.poll(() => items.count()).toBe(named.length);
	await expect(items.first()).toContainText(named[0].name);

	await page.getByRole('button', { name: 'Clear search' }).click();
	await expect.poll(() => items.count()).toBeGreaterThan(named.length);
	expect(await items.count()).toBe(restored);

	const stop = stops[0];
	await page.getByRole('searchbox').fill(stop.id);
	await expect.poll(() => items.count()).toBe(1);
	await expect(items).toContainText(stop.name);

	await page.getByRole('searchbox').fill(route[0]);
	const names = items.locator('.text-lg');
	await expect(names.first()).toHaveText(routeStops[0].name);
	const expectedNames = new Set(routeStops.map((stop) => stop.name));
	for (const name of await names.allTextContents()) {
		expect(expectedNames.has(name.trim())).toBe(true);
	}

	// Only viewport rows plus overscan are mounted. Reach the final stop by scrolling.
	const viewport = page.locator('main .overflow-y-auto');
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
