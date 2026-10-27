import { createRawSnippet } from 'svelte';

import { SvelteURL } from 'svelte/reactivity';

import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page as browserPage } from 'vitest/browser';

import { chartFixture, chartNow } from '../lib/charts/fixtures';
import { page } from '../lib/test/page.svelte';
import { fixtureFetcher } from '../lib/test/transit-fixture';
import Layout from './+layout.svelte';

let fail = true;

beforeEach(() => {
	fail = true;
	const fixture = chartFixture('njt_bus', 0);
	const respond = fixtureFetcher({
		catalogs: { njt_bus: fixture.catalog },
		services: { njt_bus: fixture.service }
	});
	vi.stubGlobal('fetch', ((input: RequestInfo | URL, init?: RequestInit) =>
		fail
			? Promise.reject(new TypeError('Connection failed'))
			: respond(input, init)) as typeof fetch);
	vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true);
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

async function coldLoad() {
	page.data.selected_sources = ['njt_bus'];
	page.url = new SvelteURL('http://localhost/?t=trip-0&r=4&src=njt_bus');
	return render(Layout, {
		children: createRawSnippet(() => ({ render: () => '<div>Page content</div>' }))
	});
}

test('a direct trip URL opens at once and recovers through the shared Retry', async () => {
	const view = await coldLoad();
	await expect
		.poll(() => page.state.modal)
		.toEqual({
			type: 'trip',
			source: 'njt_bus',
			route_id: '4',
			id: 'trip-0'
		});
	const retry = view.getByRole('button', { name: /Retry updates/ });
	await expect.element(retry).toBeVisible();
	await expect
		.element(view.getByRole('dialog').getByRole('button', { name: /Retry updates/ }))
		.toBeVisible();
	fail = false;
	await view
		.getByRole('dialog')
		.getByRole('button', { name: /Retry updates/ })
		.click();
	await expect.element(retry).not.toBeInTheDocument();
	await expect.element(view.getByText(/Arrivals unavailable/)).not.toBeInTheDocument();
	await expect.element(view.getByRole('dialog')).toBeVisible();
	await view.unmount();
});

for (const [width, height] of [
	[320, 480],
	[390, 600],
	[390, 844],
	[1000, 700]
]) {
	test(`dense trip sections preserve the warning and three visible stops at ${width}×${height}`, async () => {
		await browserPage.viewport(width, height);
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		const fixture = chartFixture('mta_subway', 0);
		const trip = fixture.trips[0];
		fixture.stops[0].name = 'Atlantic Av–Barclays Ctr';
		fixture.stops[1].name = 'Lexington Av/59 St–Lexington Avenue';
		fixture.stops[2].name = 'Jamaica Center–Parsons/Archer';

		const stop = fixture.stops[0];
		if (trip.data.source !== 'mta_subway' || stop.data.source !== 'mta_subway')
			throw new Error('Expected subway');
		trip.data.consist = { car_count: 10, car_length_feet: 51 };
		stop.data.platform_edges = [
			{
				id: 'platform',
				length_ft: 600,
				car_markers: [
					{
						marked_as: '10',
						is_opto: false,
						consist_length_ft: 510,
						position_ft: 0,
						direction: 'NORTH'
					}
				],
				egress_points: Array.from({ length: 24 }, (_, id) => ({
					id,
					position_ft: 100,
					egress_type: 'STAIRCASE',
					vertical_direction: 'UP',
					street_exits: [
						{ id, intersection: [`Exit ${id}`, 'Long platform passage'], accessible: false }
					]
				}))
			}
		];
		fixture.service.routes['4'].calls.platform_edges = fixture.service.routes['4'].calls.stop.map(
			() => ['platform']
		);
		for (let i = 0; i < 6; i++) {
			const id = `X${i}`;
			fixture.catalog.routes.push({ ...fixture.route, id, short_name: id });
			stop.routes.push({ ...stop.routes[0], route_id: id });
			fixture.service.routes[id] = {
				trips: [
					{ ...trip, id: `${id}-early` },
					{ ...trip, id: `${id}-later` }
				],
				stops: [stop.id],
				calls: {
					trip: [0, 1],
					stop: [0, 0],
					sequence: [0, 0],
					arrival: [chartNow / 1000 + 600, chartNow / 1000 + 1200],
					departure: [chartNow / 1000 + 600, chartNow / 1000 + 1200]
				}
			};
		}
		const respond = fixtureFetcher({
			catalogs: { mta_subway: fixture.catalog },
			services: { mta_subway: fixture.service }
		});
		vi.stubGlobal('fetch', ((input: RequestInfo | URL, init?: RequestInit) =>
			String(input).includes('/alerts/')
				? Promise.reject(new TypeError('Connection failed'))
				: respond(input, init)) as typeof fetch);
		page.data.selected_sources = ['mta_subway'];
		page.url = new SvelteURL('http://localhost/?t=trip-0&r=4&src=mta_subway');
		const view = await render(Layout, {
			children: createRawSnippet(() => ({ render: () => '<div>Page content</div>' }))
		});
		try {
			await expect
				.element(view.getByRole('region', { name: 'Platform exits at arrival', exact: true }))
				.toBeVisible();
			const retry = view.getByRole('dialog').getByRole('button', { name: /Retry updates/ });
			await expect.element(retry).toBeInTheDocument();
			await expect
				.poll(() => {
					const control = retry.element().getBoundingClientRect();
					const wrapper = retry.element().parentElement!.getBoundingClientRect();
					const dialog = view.getByRole('dialog').element().getBoundingClientRect();
					return (
						control.height >= 36 &&
						wrapper.height >= control.height &&
						control.top >= dialog.top &&
						control.bottom <= wrapper.bottom + 0.5
					);
				})
				.toBe(true);
			await view.getByText('Show more', { exact: true }).click();
			await view.getByText('Later departures', { exact: true }).click();
			await expect
				.element(view.getByRole('button', { name: /^X5 to/ }).first())
				.toBeInTheDocument();

			await expect
				.poll(() => {
					const dialog = view.getByRole('dialog').element();
					dialog.scrollTop = 0;
					const bounds = dialog.getBoundingClientRect();
					return fixture.stops.slice(0, 3).every(({ name }) => {
						const button = view
							.getByRole('button', { name: `Open ${name} stop modal`, exact: true })
							.element();
						const row = button.getBoundingClientRect();
						const list = button.parentElement!.parentElement!.getBoundingClientRect();
						return (
							row.top >= Math.max(bounds.top, list.top) &&
							row.bottom <= Math.min(bounds.bottom, list.bottom) + 0.5
						);
					});
				})
				.toBe(true);
			const platforms = view
				.getByRole('region', { name: 'Train and platform exits; scroll for more' })
				.element();
			expect(platforms.scrollHeight).toBeGreaterThan(platforms.clientHeight);
			const transfers = view.getByRole('region', { name: 'Transfers after arrival' }).element();
			expect(transfers.scrollHeight).toBeGreaterThan(transfers.clientHeight);
			platforms.scrollTop = platforms.scrollHeight;
			transfers.scrollTop = transfers.scrollHeight;
			expect(platforms.scrollTop).toBeGreaterThan(0);
			expect(transfers.scrollTop).toBeGreaterThan(0);
		} finally {
			await view.unmount();
		}
	});
}

test('a catalog revalidated by the service worker updates the open app', async () => {
	fail = false;
	const view = await coldLoad();
	await expect.element(view.getByRole('button', { name: 'Open route 4' })).toBeVisible();
	const { catalog } = chartFixture('njt_bus', 0);
	const renamed = { ...catalog, routes: [{ ...catalog.routes[0], short_name: 'Z9' }] };
	navigator.serviceWorker.dispatchEvent(
		new MessageEvent('message', { data: { type: 'catalog', body: renamed } })
	);
	await expect.element(view.getByRole('button', { name: 'Open route Z9' })).toBeVisible();
	await view.unmount();
});

test('service-worker catalog replacement wins over an older pending fetch', async () => {
	fail = false;
	const fixture = chartFixture('njt_bus', 0);
	const respond = fixtureFetcher({
		catalogs: { njt_bus: fixture.catalog },
		services: { njt_bus: fixture.service }
	});
	let release!: () => void;
	const ready = new Promise<void>((resolve) => {
		release = resolve;
	});
	vi.stubGlobal('fetch', (async (input: RequestInfo | URL, init?: RequestInit) => {
		if (String(input).includes('/catalog/')) await ready;
		return respond(input, init);
	}) as typeof fetch);
	const view = await coldLoad();
	const renamed = {
		...fixture.catalog,
		routes: [{ ...fixture.catalog.routes[0], short_name: 'Z9' }]
	};
	navigator.serviceWorker.dispatchEvent(
		new MessageEvent('message', { data: { type: 'catalog', body: renamed } })
	);
	await expect.element(view.getByRole('button', { name: 'Open route Z9' })).toBeVisible();
	release();
	// Wait for the old body to be read as well as the new native snapshot to render.
	await new Promise((resolve) => setTimeout(resolve, 0));
	await expect.element(view.getByRole('button', { name: 'Open route Z9' })).toBeVisible();
	await expect.element(view.getByRole('button', { name: 'Open route 4' })).not.toBeInTheDocument();
	await view.unmount();
});

test('reconnecting retries failed reads without a click', async () => {
	const view = await coldLoad();
	await expect.element(view.getByRole('button', { name: /Retry updates/ })).toBeVisible();
	fail = false;
	window.dispatchEvent(new Event('online'));
	await expect.element(view.getByRole('button', { name: /Retry updates/ })).not.toBeInTheDocument();
	await view.unmount();
});

for (const source of ['njt_bus', 'mta_subway'] as const) {
	test(`a ${source} route alert dialog on the map uses catalogs without passenger requests`, async () => {
		const fixture = chartFixture(source, 0);
		const respond = fixtureFetcher({ catalogs: { [source]: fixture.catalog }, services: {} });
		const requests: string[] = [];
		vi.stubGlobal('fetch', ((input: RequestInfo | URL, init?: RequestInit) => {
			requests.push(String(input));
			return respond(input, init);
		}) as typeof fetch);
		page.data.selected_sources = [source];
		page.url = new SvelteURL(`http://localhost/map?r=4&src=${source}`);
		const view = await render(Layout, {
			children: createRawSnippet(() => ({ render: () => '<div>Map content</div>' }))
		});
		await expect.element(view.getByRole('dialog').getByText('No alerts')).toBeVisible();
		expect(requests.some((url) => url.includes('/service/'))).toBe(false);
		expect(page.state.modal).toEqual({ type: 'route', source, id: '4' });
		// The saved route remains usable until a revalidated catalog actually removes it.
		navigator.serviceWorker.dispatchEvent(
			new MessageEvent('message', {
				data: { type: 'catalog', body: { ...fixture.catalog, routes: [] } }
			})
		);
		await expect.poll(() => page.state.modal ?? null).toBeNull();
		expect(requests.some((url) => url.includes('/service/'))).toBe(false);
		await view.unmount();
	});
}

test('historical layouts load background data at the URL time and follow URL changes', async () => {
	vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
	const now = vi.spyOn(Date, 'now').mockReturnValue(1_780_000_000_000);
	fail = false;
	const fixture = chartFixture('mta_subway', 0);
	const respond = fixtureFetcher({
		catalogs: { mta_subway: fixture.catalog },
		services: { mta_subway: fixture.service }
	});
	const requests: URL[] = [];
	vi.stubGlobal('fetch', ((input: RequestInfo | URL, init?: RequestInit) => {
		requests.push(new URL(String(input), location.origin));
		return respond(input, init);
	}) as typeof fetch);
	page.data.selected_sources = ['mta_subway'];
	page.data.at = '100';
	page.url = new SvelteURL('http://localhost/?at=100');
	const view = await render(Layout, {
		children: createRawSnippet(() => ({ render: () => '<div>Page content</div>' }))
	});
	const realtime = () => requests.filter((url) => !url.pathname.includes('/catalog/'));
	await expect.poll(() => realtime().length).toBe(2);
	expect(realtime().map((url) => url.searchParams.get('at'))).toEqual(['100', '100']);
	page.url = new SvelteURL('http://localhost/?at=200');
	await expect.poll(() => realtime().length).toBe(4);
	expect(
		realtime()
			.slice(2)
			.map((url) => url.searchParams.get('at'))
	).toEqual(['200', '200']);
	await vi.advanceTimersByTimeAsync(31_000);
	expect(realtime()).toHaveLength(4);
	page.url = new SvelteURL('http://localhost/');
	await expect.poll(() => realtime().length).toBe(6);
	expect(
		realtime()
			.slice(4)
			.map((url) => url.searchParams.get('at'))
	).toEqual([null, null]);
	now.mockReturnValue(1_780_000_031_000);
	await vi.advanceTimersByTimeAsync(31_000);
	await expect.poll(() => realtime().length).toBe(8);
	page.url = new SvelteURL('http://localhost/?at=100');
	await expect
		.element(view.getByRole('link', { name: 'Settings' }))
		.toHaveAttribute('href', '/settings?at=100');
	now.mockReturnValue(1_780_000_062_000);
	await vi.advanceTimersByTimeAsync(31_000);
	expect(realtime()).toHaveLength(8);
	await view.unmount();
});

for (const query of [
	'?s=missing&src=njt_bus',
	'?r=missing&src=njt_bus',
	'?t=missing&r=4&src=njt_bus',
	'?t=trip-0&src=njt_bus',
	'?r=4&src=invalid'
]) {
	test(`silently dismisses invalid modal URL ${query}`, async () => {
		fail = false;
		page.data.selected_sources = ['njt_bus'];
		page.data.at = '0';
		page.url = new SvelteURL(`http://localhost/${query}&at=0&keep=value`);
		const view = await render(Layout, {
			children: createRawSnippet(() => ({ render: () => '<div>Page content</div>' }))
		});
		await expect.poll(() => page.state.modal ?? null).toBeNull();
		await expect.poll(() => page.url.searchParams.has('src')).toBe(false);
		expect(page.url.searchParams.get('at')).toBe('0');
		expect(page.url.searchParams.get('keep')).toBe('value');
		await expect
			.element(view.getByRole('button', { name: /Retry updates/ }))
			.not.toBeInTheDocument();
		await view.unmount();
	});
}
