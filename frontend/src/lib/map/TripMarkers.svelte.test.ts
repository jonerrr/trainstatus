import '../../app.css';

import type { Source } from '#lib/client/index.js';

import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';

import MapHarness from '../test/MapHarness.svelte';
import { renderUnitTableFromIPC } from './trajectory';
import { trajectoryFixture } from './trajectory-fixture';

afterEach(() => vi.restoreAllMocks());

function tables({
	lengthM = 20,
	color = [0, 100, 200],
	source = 'mta_subway',
	moving = false
}: {
	lengthM?: number;
	color?: [number, number, number];
	source?: Source;
	moving?: boolean;
} = {}) {
	return new Map([
		[
			source,
			renderUnitTableFromIPC(
				trajectoryFixture(['vehicle-1'], 200, {
					source,
					lengthM,
					color,
					positions: [
						[0, -0.001],
						[0, 0.001]
					],
					distances: [0, 200],
					motion: [{ t0: 100, t1: 200, a: moving ? 0 : 100, b: moving ? 200 : 0, c: 0, d: 0 }]
				})
			)
		]
	]);
}

test('same-ID refreshes update real GPU size and color, and detail changes update picking', async () => {
	await page.viewport(1100, 800);
	const view = await render(MapHarness, { tables: tables(), at: 150, railDetail: true });
	await expect.poll(() => view.component.read().map?.loaded()).toBe(true);
	const map = view.component.read().map!;
	map.jumpTo({ zoom: 19 });
	const center = map.project([0, 0]);
	const pixelsPerMeter = Math.abs(map.project([0, 1 / 111319.49079327358]).y - center.y);
	const pick = (dy: number) =>
		view.component.read().picker?.pickClosest({ x: center.x, y: center.y + dy }, 0);
	await expect.poll(() => pick(0)?.tripId).toBe('trip-1');
	// Initially outside the 20m body; inside after the same vehicle grows to 40m.
	await expect.poll(() => pick(12 * pixelsPerMeter)).toBeNull();
	await view.rerender({ tables: tables({ lengthM: 40, color: [200, 50, 0] }) });
	await expect.poll(() => pick(12 * pixelsPerMeter)?.tripId).toBe('trip-1');
	const pixel = () => {
		const canvas = map.getCanvas();
		const gl = canvas.getContext('webgl2')!;
		const ratio = canvas.width / canvas.clientWidth;
		// Sample the route-colored ring, clear of the white body and dark casing.
		const bytes = new Uint8Array(4);
		gl.readPixels(
			Math.round(center.x * ratio),
			Math.round((canvas.clientHeight - center.y + 16.9 * pixelsPerMeter) * ratio),
			1,
			1,
			gl.RGBA,
			gl.UNSIGNED_BYTE,
			bytes
		);
		return [...bytes.slice(0, 3)];
	};
	await expect.poll(pixel).toEqual([200, 50, 0]);
	await view.rerender({ railDetail: false });
	await expect.poll(() => pick(12 * pixelsPerMeter)).toBeNull();
	await expect.poll(() => pick(0)?.tripId).toBe('trip-1');
});

test('transparent atlas padding does not pick a vehicle', async () => {
	await page.viewport(1100, 800);
	const view = await render(MapHarness, { tables: tables() });
	await expect.poll(() => view.component.read().map?.loaded()).toBe(true);
	const pick = (x: number, y: number) => view.component.read().picker?.pickClosest({ x, y }, 0);
	await expect.poll(() => pick(400, 250)?.tripId).toBe('trip-1');
	// Inside the 17.6 x 22px atlas quad but outside the directional silhouette.
	expect(pick(407, 240)).toBeNull();
});

test('live animation moves GPU picking and changing to historical time stops advancing', async () => {
	await page.viewport(1100, 800);
	let now = 150_000;
	vi.spyOn(Date, 'now').mockImplementation(() => now);
	const view = await render(MapHarness, { tables: tables({ moving: true }), at: undefined });
	await expect.poll(() => view.component.read().map?.loaded()).toBe(true);
	const map = view.component.read().map!;
	map.jumpTo({ zoom: 17 });
	const pick = (lat: number) => view.component.read().picker?.pickClosest(map.project([0, lat]), 0);
	await expect.poll(() => pick(0)?.tripId).toBe('trip-1');
	now = 175_000;
	await expect.poll(() => pick(0.0005)?.tripId).toBe('trip-1');
	expect(pick(0)).toBeNull();
	await view.rerender({ at: 150 });
	await expect.poll(() => pick(0)?.tripId).toBe('trip-1');
	now = 190_000;
	await new Promise<void>((resolve) =>
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
	);
	expect(pick(0)?.tripId).toBe('trip-1');
	expect(pick(0.0005)).toBeNull();
});
