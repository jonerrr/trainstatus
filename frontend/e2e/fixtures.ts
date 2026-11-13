import { test as base, expect } from '@playwright/test';

import type { Catalog, CatalogStop } from '../src/lib/client/index.js';

/** Pick a name prefix that resolves to one stop, without relying on a fixed live ID. */
export function searchableStop(
	catalog: Catalog,
	excludeStopIds: readonly string[] = []
): { stop: CatalogStop; query: string } {
	const routeIds = new Set(catalog.routes.map((route) => route.id));
	const tokens = catalog.stops.map((stop) => ({
		stop,
		words: stop.name.toLowerCase().split(/[^a-z0-9]+/)
	}));
	for (const { stop, words } of tokens) {
		if (routeIds.has(stop.id) || excludeStopIds.includes(stop.id)) continue;
		for (const word of words) {
			if (word.length < 5) continue;
			const matchesAnotherStop = tokens.some(
				(other) =>
					other.stop.id !== stop.id && other.words.some((candidate) => candidate.startsWith(word))
			);
			if (!matchesAnotherStop) return { stop, query: word };
		}
	}

	throw new Error('expected a uniquely searchable stop name');
}

export const test = base.extend({
	page: async ({ page }, use) => {
		await page.context().clearCookies();
		await page.context().addInitScript(() => {
			const flag = 'e2e-storage-cleared';
			if (sessionStorage.getItem(flag)) return;
			localStorage.clear();
			sessionStorage.setItem(flag, '1');
		});
		await page.goto('/');
		await expect(page.getByRole('navigation')).toBeVisible();
		await use(page);
	}
});
export { expect };
