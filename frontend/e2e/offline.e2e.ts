import { expect, test } from './fixtures';

// The shared fixtures block service workers; this contract needs the real one.
test.use({ serviceWorkers: 'allow' });

test('a reopened offline app navigates to an unvisited page using saved catalogs', async ({
	page,
	context
}) => {
	await page.evaluate(async () => {
		await navigator.serviceWorker.ready;
	});
	// The first load predates the worker; reload so the page and its catalog reads go through it.
	await page.reload();
	await expect.poll(() => page.evaluate(() => !!navigator.serviceWorker.controller)).toBe(true);
	await expect
		.poll(() =>
			page.evaluate(async () => (await (await caches.open('catalogs')).keys()).map((r) => r.url))
		)
		.toContainEqual(expect.stringContaining('/api/v1/catalog/mta_subway'));

	await context.setOffline(true);
	await page.goto('/stops');
	await expect.poll(() => page.locator('main .list-item').count()).toBeGreaterThan(1);
	await expect(page.getByRole('status').first()).toHaveText(/Offline/);

	const saved = await page.evaluate(async () => {
		const urls: string[] = [];
		for (const name of await caches.keys()) {
			for (const request of await (await caches.open(name)).keys()) urls.push(request.url);
		}
		return { names: await caches.keys(), urls };
	});
	expect(saved.names.every((name) => name === 'catalogs' || name.startsWith('shell-'))).toBe(true);
	const api = saved.urls.filter((url) => new URL(url).pathname.startsWith('/api/'));
	expect(api.every((url) => new URL(url).pathname.startsWith('/api/v1/catalog/'))).toBe(true);
	await context.setOffline(false);
});
