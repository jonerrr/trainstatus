import { tick } from 'svelte';

import { SvelteURL } from 'svelte/reactivity';

import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import { stop_pins, trip_pin_key, trip_pins } from '#lib/pins.svelte.js';
import { layout_sources } from '#lib/source_preferences.svelte.js';
import HomeLayoutHarness from '#lib/test/HomeLayoutHarness.svelte';
import { page } from '#lib/test/page.svelte.js';
import { fixtureFetcher } from '#lib/test/transit-fixture.js';
import { open_modal } from '#lib/url_params.svelte.js';

import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

beforeEach(() => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true);
	vi.spyOn(navigator.geolocation, 'watchPosition').mockImplementation((success) => {
		success({ coords: { latitude: 0, longitude: 0 } } as GeolocationPosition);
		return 1;
	});
	vi.spyOn(navigator.geolocation, 'clearWatch').mockImplementation(() => {});
	stop_pins.current = { mta_subway: ['a'], mta_bus: ['a'], njt_bus: ['a'] };
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

test.each([
	['stop', 'mta_subway', 'catalog-first'],
	['trip', 'mta_subway', 'catalog-first'],
	['stop', 'njt_bus', 'catalog-first'],
	['trip', 'njt_bus', 'catalog-first'],
	['stop', 'mta_subway', 'service-first'],
	['trip', 'mta_subway', 'service-first'],
	['stop', 'njt_bus', 'service-first'],
	['trip', 'njt_bus', 'service-first']
] as const)('a cold %s link for %s renders with home readers (%s)', async (kind, source, order) => {
	const fixtures = ['mta_subway', 'njt_bus'].map((source) =>
		chartFixture(source as 'mta_subway' | 'njt_bus')
	);
	const respond = fixtureFetcher({
		catalogs: Object.fromEntries(fixtures.map((f) => [f.catalog.source, f.catalog])),
		services: Object.fromEntries(fixtures.map((f) => [f.service.source, f.service]))
	});
	let releaseCatalog!: () => void;
	let releaseService!: () => void;
	const catalogReady = new Promise<void>((resolve) => {
		releaseCatalog = resolve;
	});
	const serviceReady = new Promise<void>((resolve) => {
		releaseService = resolve;
	});
	const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
		if (String(input).includes('/catalog/')) await catalogReady;
		if (String(input).includes('/service/')) await serviceReady;
		return respond(input, init);
	});
	vi.stubGlobal('fetch', fetcher);
	// Bus links must add a source absent from the saved preferences.
	page.data.selected_sources = layout_sources(
		source === 'njt_bus' ? 'mta_subway' : 'mta_subway,njt_bus',
		source
	);
	page.url = new SvelteURL(
		`http://localhost/?${kind === 'stop' ? 's=a' : 't=trip-0&r=4'}&src=${source}`
	);
	const view = await render(HomeLayoutHarness);
	try {
		const dialog = view.getByRole('dialog');
		await expect.element(dialog).toBeVisible();
		if (order === 'catalog-first') {
			releaseCatalog();
			await expect.element(dialog.getByRole('button', { name: 'Open route 4' })).toBeVisible();
			releaseService();
		} else {
			releaseService();
			await tick();
			releaseCatalog();
		}
		await expect.element(view.getByRole('heading', { name: 'Pinned Trips' })).toBeVisible();
		await expect.element(view.getByRole('heading', { name: /^Nearby Stops/ })).toBeVisible();
		if (kind === 'stop') {
			await expect.element(dialog.getByRole('heading', { name: 'a', exact: true })).toBeVisible();
			await expect.element(dialog.getByText('1m', { exact: true }).first()).toBeVisible();
		} else {
			await expect.element(dialog.getByText('d', { exact: true }).first()).toBeVisible();
		}
		expect(
			fetcher.mock.calls.filter(([input]) => String(input).startsWith(`/api/v1/service/${source}`))
		).toHaveLength(1);
		await dialog.getByRole('button', { name: 'Close modal' }).click();
		await expect.element(dialog).not.toBeInTheDocument();
		if (kind === 'stop') open_modal({ type: 'stop', source, id: 'a' });
		else open_modal({ type: 'trip', source, route_id: '4', id: 'trip-0' });
		await expect.element(dialog).toBeVisible();
		expect(
			fetcher.mock.calls.filter(([input]) => String(input).startsWith(`/api/v1/service/${source}`))
		).toHaveLength(1);

		for (const fixture of fixtures) {
			const calls = fixture.service.routes['4'].calls;
			calls.arrival = calls.arrival.map((time) => time + 300);
			calls.departure = calls.departure.map((time) => time + 300);
		}
		vi.mocked(Date.now).mockReturnValue(chartNow + 31_000);
		document.dispatchEvent(new Event('visibilitychange'));
		await expect
			.element(dialog.getByText(kind === 'stop' ? '5m' : '8m', { exact: true }).first())
			.toBeVisible();
		expect(
			fetcher.mock.calls.filter(([input]) => String(input).startsWith(`/api/v1/service/${source}`))
		).toHaveLength(2);
	} finally {
		releaseCatalog();
		releaseService();
		await view.unmount();
	}
});
