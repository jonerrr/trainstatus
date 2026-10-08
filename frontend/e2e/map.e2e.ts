import type { Page } from '@playwright/test';

import type { Route } from '../src/lib/client/index.js';
import { expect, test } from './fixtures';

test.setTimeout(90_000);

async function pan(page: Page, touch: boolean, cancel = false) {
	const canvas = page.locator('.maplibregl-canvas');
	const box = await canvas.boundingBox();
	if (!box) throw new Error('Map canvas is unavailable');
	const before = await page.locator('[data-map-center]').getAttribute('data-map-center');
	const x = box.x + box.width * 0.8,
		y = box.y + box.height * 0.25;
	if (touch) {
		const session = await page.context().newCDPSession(page);
		await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
		for (let step = 1; step <= 12; step++) {
			await session.send('Input.dispatchTouchEvent', {
				type: 'touchMove',
				touchPoints: [{ x: x - step * 6, y: y + step * 2 }]
			});
		}
		await session.send('Input.dispatchTouchEvent', {
			type: cancel ? 'touchCancel' : 'touchEnd',
			touchPoints: []
		});
		await session.detach();
	} else {
		await page.mouse.move(x, y);
		await page.mouse.down();
		await page.mouse.move(x - 100, y + 30, { steps: 15 });
		await page.mouse.up();
	}
	await expect
		.poll(() => page.locator('[data-map-center]').getAttribute('data-map-center'))
		.not.toBe(before);
}

test('map remains pannable across filters, layers, modal dismissal and navigation', async ({
	page,
	isMobile
}) => {
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	const style = await page.request.get('/martin/style/dark-matter.json');
	expect(style.ok(), 'Martin must serve the map style').toBe(true);
	await page.goto('/map');
	await expect(page.locator('[data-map-ready="true"]')).toBeVisible();
	await pan(page, isMobile);
	await page.getByRole('button', { name: 'Filters', exact: true }).click();
	await pan(page, isMobile);
	await expect(page.getByRole('dialog', { name: 'Filters', exact: true })).toBeVisible();
	for (const name of ['Trips', 'Stops', 'Routes']) {
		await page.getByRole('checkbox', { name, exact: true }).uncheck();
		await page.getByRole('checkbox', { name, exact: true }).check();
	}
	await page.getByRole('button', { name: 'Close filters' }).click();
	await pan(page, isMobile);
	if (isMobile) {
		await pan(page, true, true);
		await pan(page, true);
	}
	const response = await page.request.get('/api/v1/routes/mta_subway');
	expect(response.ok()).toBe(true);
	const routes: Route[] = await response.json();
	expect(routes.length).toBeGreaterThan(0);
	await page.goto(`/map?r=${encodeURIComponent(routes[0].id)}&src=mta_subway`);
	await expect(page.getByRole('dialog', { name: 'Transit details' })).toBeVisible();
	await page.getByRole('button', { name: 'Close modal' }).click();
	await expect(page.getByRole('dialog', { name: 'Transit details' })).toBeHidden();
	await expect(page.locator('[data-map-ready="true"]')).toBeVisible();
	await pan(page, isMobile);
	await page.getByRole('link', { name: 'Home', exact: true }).click();
	await page.getByRole('link', { name: 'Map', exact: true }).click();
	await expect(page.locator('[data-map-ready="true"]')).toBeVisible();
	await pan(page, isMobile);
	expect(errors).toEqual([]);
});
