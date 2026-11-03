import { SvelteURL } from 'svelte/reactivity';

import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import { route_pins, stop_pins, trip_pin_key, trip_pins } from '#lib/pins.svelte.js';
import HomeLayoutHarness from '#lib/test/HomeLayoutHarness.svelte';
import { page } from '#lib/test/page.svelte.js';
import { fixtureFetcher } from '#lib/test/transit-fixture.js';

import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

beforeEach(() => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true);
	vi.spyOn(navigator.geolocation, 'watchPosition').mockReturnValue(1);
	vi.spyOn(navigator.geolocation, 'clearWatch').mockImplementation(() => {});
	stop_pins.current = { mta_subway: [], mta_bus: [], njt_bus: [] };
	route_pins.current = { mta_subway: [], mta_bus: [], njt_bus: [] };
	trip_pins.current = {
		mta_subway: [trip_pin_key('4', 'trip-0')],
		mta_bus: [trip_pin_key('4', 'trip-0')],
		njt_bus: [trip_pin_key('4', 'trip-0')]
	};
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

test.each(['mta_subway', 'mta_bus', 'njt_bus'] as const)(
	'%s removes absent trip pins after successful live service, preserving pins during loading and failures',
	async (source) => {
		const fixture = chartFixture(source);
		const active = trip_pin_key('4', 'trip-0');
		const saved = {
			...trip_pins.current,
			[source]: [
				active,
				...Array.from({ length: 8 }, (_, i) => trip_pin_key('4', `ended-${i}`)),
				trip_pin_key('9', 'ended')
			]
		};
		trip_pins.current = saved;
		page.data.selected_sources = [source];
		const respond = fixtureFetcher({
			catalogs: { [source]: fixture.catalog },
			services: { [source]: fixture.service }
		});
		let release!: () => void;
		const ready = new Promise<void>((resolve) => {
			release = resolve;
		});
		let fail = true;
		const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			if (String(input).includes('/service/')) {
				await ready;
				if (fail) return new Response(null, { status: 503 });
			}
			return respond(input, init);
		});
		vi.stubGlobal('fetch', fetcher);
		const view = await render(HomeLayoutHarness);
		try {
			await expect
				.poll(() => fetcher.mock.calls.some(([input]) => String(input).includes('/service/')))
				.toBe(true);
			expect(trip_pins.current).toEqual(saved);
			release();
			const retry = view.getByRole('button', { name: /Retry updates/ });
			await expect.element(retry).toBeVisible();
			expect(trip_pins.current).toEqual(saved);
			fail = false;
			vi.mocked(Date.now).mockReturnValue(chartNow + 6_000);
			document.dispatchEvent(new Event('visibilitychange'));
			const expected = { ...saved, [source]: [active] };
			await expect.poll(() => trip_pins.current).toEqual(expected);
			expect(JSON.parse(localStorage.getItem('tripRoutePins')!)).toEqual(expected);
			await expect.element(view.getByRole('heading', { name: 'Pinned Trips' })).toBeVisible();

			// A failed refresh retains the active pin; a successful empty replacement removes it.
			fail = true;
			vi.mocked(Date.now).mockReturnValue(chartNow + 37_000);
			document.dispatchEvent(new Event('visibilitychange'));
			await expect.element(retry).toBeVisible();
			expect(trip_pins.current).toEqual(expected);
			fixture.service.routes['4'] = {
				trips: [],
				stops: [],
				calls: { trip: [], stop: [], sequence: [], arrival: [], departure: [] }
			};
			fail = false;
			vi.mocked(Date.now).mockReturnValue(chartNow + 43_000);
			document.dispatchEvent(new Event('visibilitychange'));
			await expect.poll(() => trip_pins.current).toEqual({ ...saved, [source]: [] });
			await expect
				.element(view.getByRole('heading', { name: 'Pinned Trips' }))
				.not.toBeInTheDocument();
		} finally {
			release();
			await view.unmount();
		}
	}
);

test('historical service never removes trip pins; returning to live checks them', async () => {
	const fixture = chartFixture('njt_bus');
	fixture.service.routes = {};
	const saved = trip_pins.current;
	page.data.selected_sources = ['njt_bus'];
	page.url = new SvelteURL(`http://localhost/?at=${chartNow / 1000 - 3600}`);
	const fetcher = vi.fn(
		fixtureFetcher({
			catalogs: { njt_bus: fixture.catalog },
			services: { njt_bus: fixture.service }
		})
	);
	vi.stubGlobal('fetch', fetcher);
	const view = await render(HomeLayoutHarness);
	try {
		await expect.element(view.getByRole('button', { name: /Trip not running/ })).toBeVisible();
		expect(trip_pins.current).toEqual(saved);
		expect(
			fetcher.mock.calls
				.filter(([input]) => String(input).includes('/service/'))
				.every(([input]) => String(input).includes('at='))
		).toBe(true);
		page.url = new SvelteURL('http://localhost/');
		await expect.poll(() => trip_pins.current).toEqual({ ...saved, njt_bus: [] });
	} finally {
		await view.unmount();
	}
});
