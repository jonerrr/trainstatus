import { tick } from 'svelte';

import { chartFixture } from '#lib/charts/fixtures.js';
import type { RouteService, Service, Source } from '#lib/client/index.js';
import { watchTrajectories } from '#lib/map/trajectories.svelte.js';
import { trajectoryFixture } from '#lib/map/trajectoryFixture.js';
import TransitDataHarness from '#lib/test/TransitDataHarness.svelte';

import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { TransitData } from './store.svelte.js';

function routeService(trip_ids: string[]): RouteService {
	return {
		trips: trip_ids.map((id) => ({
			id,
			vehicle_id: `vehicle-${id}`,
			direction: 0,
			data: { source: 'mta_bus' },
			status: null
		})),
		stops: ['S1'],
		calls: {
			trip: trip_ids.map((_, i) => i),
			stop: trip_ids.map(() => 0),
			sequence: trip_ids.map(() => 1),
			arrival: trip_ids.map(() => 1_780_000_000),
			departure: trip_ids.map(() => 1_780_000_030)
		}
	};
}

interface Call {
	url: URL;
	signal: AbortSignal;
	respond: (response: Response) => void;
}

/** Fetcher whose responses the test supplies explicitly, in request order. */
function transport() {
	const calls: Call[] = [];
	const json = (body: unknown, status = 200) =>
		new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });
	const fetcher = ((input: RequestInfo | URL, init?: RequestInit) =>
		new Promise<Response>((resolve, reject) => {
			const url = new URL(String(input), location.origin);
			if (url.pathname.startsWith('/api/v1/alerts/')) return resolve(json([]));
			if (url.pathname.startsWith('/api/v1/catalog/')) {
				const source = url.pathname.split('/').pop();
				return resolve(json({ source, routes: [], stops: [] }));
			}
			const signal = init!.signal!;
			signal.addEventListener('abort', () => reject(signal.reason));
			calls.push({ url, signal, respond: resolve });
		})) as typeof fetch;
	const routes = (call: Call) => call.url.searchParams.get('route_ids')?.split(',') ?? [];
	const service = (source: Source, routes: Record<string, string[]>): Service => ({
		source,
		routes: Object.fromEntries(
			Object.entries(routes).map(([id, trips]) => [id, routeService(trips)])
		)
	});
	return { calls, fetcher, routes, json, service };
}

let clock = 1_780_000_000_000;
const created: TransitData[] = [];

function store(fetcher: typeof fetch, sources: Source[] = ['mta_bus'], history_limit = 6) {
	const data = new TransitData({ sources, fetcher, history_limit, batch_ms: 0 });
	created.push(data);
	return data;
}

test('each app mount loads catalogs', async () => {
	const catalog = chartFixture('mta_bus').catalog;
	const fetcher: typeof fetch = async (input) => {
		if (String(input).includes('/catalog/')) {
			return new Response(JSON.stringify(catalog));
		}
		return new Response('[]');
	};
	const first = store(fetcher);
	await settle();
	expect(first.peekCatalog('mta_bus')?.stops).toEqual(catalog.stops);
	first.destroy();
	const second = store(fetcher);
	await settle();
	expect(second.peekCatalog('mta_bus')?.stops).toEqual(catalog.stops);
});

test('successful map polls retain source age in the shared warning until a new commit', async () => {
	const http = transport();
	const data = store(http.fetcher);
	let committed_at = clock - 91_000;
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: undefined },
		() => {},
		async () =>
			new Response(trajectoryFixture(), {
				headers: { 'x-source-updated-at': String(committed_at / 1000) }
			}),
		(update) => data.track(update)
	);
	try {
		await stop.refresh();
		document.dispatchEvent(new Event('visibilitychange'));
		expect(data.status.warning).toBe(true);
		committed_at = clock;
		await stop.refresh();
		document.dispatchEvent(new Event('visibilitychange'));
		expect(data.status.warning).toBe(false);
		clock += 91_000;
		await stop.refresh();
		document.dispatchEvent(new Event('visibilitychange'));
		await settle();
		expect(data.status.warning).toBe(true);
		committed_at = clock;
		await stop.refresh();
		document.dispatchEvent(new Event('visibilitychange'));
		expect(data.status.warning).toBe(false);
		expect(http.calls).toHaveLength(0);
	} finally {
		stop();
	}
});

const text = (route: string) =>
	[...document.querySelectorAll(`[data-route="${route}"]`)].map((node) => node.textContent);
const pending = (route: string) => document.querySelector(`[data-pending="${route}"]`) !== null;

async function settle() {
	for (let i = 0; i < 5; i++) {
		await new Promise((resolve) => setTimeout(resolve, 0));
		await tick();
	}
}

beforeEach(() => {
	vi.spyOn(Date, 'now').mockImplementation(() => clock);
	vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true);
});

afterEach(() => {
	vi.restoreAllMocks();
	for (const data of created.splice(0)) data.destroy();
	vi.useRealTimers();
	clock = 1_780_000_000_000;
});

test('fast-changing bus rows wait for a pause and fetch only routes still being read', async () => {
	vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
	const http = transport();
	const data = new TransitData({ sources: ['mta_bus'], fetcher: http.fetcher });
	created.push(data);
	const view = await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [
			{ id: 'pinned', route: 'A' },
			{ id: 'row', route: 'B' }
		]
	});
	await tick();
	for (const route of ['C', 'D']) {
		await vi.advanceTimersByTimeAsync(80);
		expect(http.calls).toHaveLength(0);
		await view.rerender({
			readers: [
				{ id: 'pinned', route: 'A' },
				{ id: 'row', route }
			]
		});
		await tick();
	}
	await view.rerender({
		readers: [
			{ id: 'pinned', route: 'A' },
			{ id: 'row', route: 'D' },
			{ id: 'next', route: 'E' }
		]
	});
	await tick();
	await vi.advanceTimersByTimeAsync(199);
	expect(http.calls).toHaveLength(0);
	// Reacquisition by the polling loop must not extend the quiet window.
	document.dispatchEvent(new Event('visibilitychange'));
	await vi.advanceTimersByTimeAsync(1);
	expect(http.calls.map(http.routes)).toEqual([['A', 'D', 'E']]);
	http.calls[0].respond(http.json(http.service('mta_bus', { A: ['a1'], D: ['d1'], E: ['e1'] })));
	await tick();
	await vi.advanceTimersByTimeAsync(0);
	expect(text('D')).toEqual(['d1']);
});

test('bus rows removed before the debounce expires make no request', async () => {
	vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
	const http = transport();
	const data = new TransitData({ sources: ['mta_bus'], fetcher: http.fetcher });
	created.push(data);
	const view = await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [{ id: 'row', route: 'A' }]
	});
	await tick();
	await view.rerender({ readers: [] });
	await tick();
	await vi.advanceTimersByTimeAsync(200);
	expect(http.calls).toHaveLength(0);
	await view.rerender({ readers: [{ id: 'row', route: 'A' }] });
	await tick();
	await vi.advanceTimersByTimeAsync(200);
	expect(http.calls.map(http.routes)).toEqual([['A']]);
});

test('bus debouncing does not delay subway service and slow changes make their own batches', async () => {
	vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
	const http = transport();
	const data = new TransitData({
		sources: ['mta_subway', 'mta_bus'],
		fetcher: http.fetcher
	});
	created.push(data);
	const view = await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [{ id: 'row', route: 'A' }]
	});
	await tick();
	await vi.advanceTimersByTimeAsync(10);
	expect(http.calls.map((call) => call.url.pathname)).toEqual(['/api/v1/service/mta_subway']);
	await vi.advanceTimersByTimeAsync(190);
	expect(http.calls.map(http.routes)).toEqual([[], ['A']]);
	await view.rerender({ readers: [{ id: 'row', route: 'B' }] });
	await tick();
	await vi.advanceTimersByTimeAsync(200);
	expect(http.calls.map(http.routes)).toEqual([[], ['A'], ['B']]);
	await view.rerender({ readers: [{ id: 'row', route: 'C' }] });
	await tick();
	data.destroy();
	await vi.advanceTimersByTimeAsync(200);
	expect(http.calls).toHaveLength(3);
});

test('duplicate and independent readers share one batched request', async () => {
	const http = transport();
	const data = store(http.fetcher);
	await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [
			{ id: '1', route: 'A' },
			{ id: '2', route: 'A' },
			{ id: '3', route: 'B' }
		]
	});
	await settle();
	expect(http.calls).toHaveLength(1);
	expect(http.calls[0].url.pathname).toBe('/api/v1/service/mta_bus');
	expect(http.routes(http.calls[0])).toEqual(['A', 'B']);
	http.calls[0].respond(http.json(http.service('mta_bus', { A: ['a1'], B: ['b1'] })));
	await settle();
	expect(text('A')).toEqual(['a1', 'a1']);
	expect(text('B')).toEqual(['b1']);
});

test('a batch response satisfies later single-route readers without a request or pending UI', async () => {
	const http = transport();
	const data = store(http.fetcher);
	const view = await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [
			{ id: '1', route: 'A' },
			{ id: '2', route: 'B' }
		]
	});
	await settle();
	http.calls[0].respond(http.json(http.service('mta_bus', { A: ['a1'], B: ['b1'] })));
	await settle();
	await view.rerender({ readers: [] });
	await view.rerender({ readers: [{ id: '3', route: 'A' }] });
	expect(pending('A')).toBe(false);
	expect(text('A')).toEqual(['a1']);
	await settle();
	expect(http.calls).toHaveLength(1);
});

test('requesting another route while one is in flight neither cancels nor waits for it', async () => {
	const http = transport();
	const data = store(http.fetcher);
	const view = await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [{ id: '1', route: 'A' }]
	});
	await settle();
	await view.rerender({ readers: [{ id: '2', route: 'B' }] });
	await settle();
	expect(http.calls.map(http.routes)).toEqual([['A'], ['B']]);
	http.calls[1].respond(http.json(http.service('mta_bus', { B: ['b1'] })));
	await settle();
	expect(text('B')).toEqual(['b1']);
	expect(http.calls[0].signal.aborted).toBe(false);
	http.calls[0].respond(http.json(http.service('mta_bus', { A: ['a1'] })));
	await view.rerender({ readers: [{ id: '3', route: 'A' }] });
	await settle();
	expect(text('A')).toEqual(['a1']);
	expect(http.calls).toHaveLength(2);
});

test('a due remount shows retained data and refreshes once in the background', async () => {
	const http = transport();
	const data = store(http.fetcher, []);
	const view = await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [{ id: '1', route: 'A' }]
	});
	await settle();
	http.calls[0].respond(http.json(http.service('mta_bus', { A: ['a1'] })));
	await settle();
	await view.rerender({ readers: [] });
	clock += 31_000;
	await view.rerender({ readers: [{ id: '2', route: 'A' }] });
	expect(text('A')).toEqual(['a1']);
	await settle();
	expect(http.calls).toHaveLength(2);
	http.calls[1].respond(http.json(http.service('mta_bus', { A: ['a2'] })));
	await settle();
	expect(text('A')).toEqual(['a2']);
	expect(data.status.warning).toBe(false);
});

test('scrolling away from an aged bus row clears its warning without waiting for a request', async () => {
	const http = transport();
	const data = store(http.fetcher, ['njt_bus']);
	const view = await render(TransitDataHarness, {
		data,
		source: 'njt_bus',
		readers: [{ id: 'row', route: 'A' }]
	});
	await settle();
	http.calls[0].respond(http.json(http.service('njt_bus', { A: [] })));
	await settle();
	expect(data.status.warning).toBe(false);
	clock += 91_000;
	document.dispatchEvent(new Event('visibilitychange'));
	await settle();
	expect(data.status.warning).toBe(true);
	await view.rerender({ readers: [] });
	await tick();
	expect(data.status.warning).toBe(false);
});

test('failed refreshes retain data and empty replacements remove stale trips', async () => {
	const http = transport();
	const data = store(http.fetcher);
	await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [{ id: '1', route: 'A' }]
	});
	await settle();
	http.calls[0].respond(http.json(http.service('mta_bus', { A: ['a1'] })));
	await settle();

	clock += 31_000;
	document.dispatchEvent(new Event('visibilitychange'));
	await settle();
	http.calls[1].respond(new Response('down', { status: 500 }));
	await settle();
	expect(text('A')).toEqual(['a1']);
	expect(data.status.warning).toBe(true);

	clock += 60_000;
	document.dispatchEvent(new Event('visibilitychange'));
	await settle();
	expect(data.status.warning).toBe(true);
	expect(text('A')).toEqual(['a1']);
	http.calls[2].respond(http.json(http.service('mta_bus', { A: [] })));
	await settle();
	expect(text('A')).toEqual(['']);
	expect(data.status.warning).toBe(false);
});

test('a failed first load ends loading and a shared Retry recovers every reader', async () => {
	const http = transport();
	const data = store(http.fetcher);
	await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [
			{ id: '1', route: 'A' },
			{ id: '2', route: 'A' }
		]
	});
	await settle();
	http.calls[0].respond(new Response('down', { status: 503 }));
	await settle();
	expect(document.querySelector('[data-failed="A"]')?.textContent).toBe('Service returned 503');
	expect(pending('A')).toBe(false);
	expect(data.status.warning).toBe(true);
	data.retry();
	await settle();
	expect(http.calls).toHaveLength(2);
	http.calls[1].respond(http.json(http.service('mta_bus', { A: ['recovered'] })));
	await settle();
	expect(text('A')).toEqual(['recovered', 'recovered']);
	expect(data.status.warning).toBe(false);
});

test('an aged source commit warns even after a successful empty fetch and Retry refreshes it', async () => {
	const http = transport();
	const data = store(http.fetcher);
	await render(TransitDataHarness, { data, source: 'mta_bus', readers: [{ id: '1', route: 'A' }] });
	await settle();
	http.calls[0].respond(
		http.json({ ...http.service('mta_bus', { A: [] }), updated_at: clock / 1000 - 91 })
	);
	await settle();
	expect(text('A')).toEqual(['']);
	expect(data.status.warning).toBe(true);
	data.retry();
	await settle();
	expect(http.calls).toHaveLength(2);
	http.calls[1].respond(
		http.json({ ...http.service('mta_bus', { A: ['fresh'] }), updated_at: clock / 1000 })
	);
	await settle();
	expect(text('A')).toEqual(['fresh']);
	expect(data.status.warning).toBe(false);
});

test('historical failures warn while mounted and successful history never ages', async () => {
	const http = transport();
	const data = store(http.fetcher);
	const view = await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [{ id: 'h1', route: 'A', at: 100 }]
	});
	await settle();
	http.calls[0].respond(new Response('down', { status: 503 }));
	await settle();
	expect(data.status.warning).toBe(true);
	data.retry();
	await settle();
	http.calls[1].respond(http.json({ ...http.service('mta_bus', { A: ['old'] }), at: 100 }));
	await settle();
	clock += 100_000;
	document.dispatchEvent(new Event('visibilitychange'));
	await settle();
	expect(data.status.warning).toBe(false);
	expect(http.calls).toHaveLength(2);
	await view.unmount();
});

test('unknown routes fail alone and the rest of the batch is requested again', async () => {
	const http = transport();
	const data = store(http.fetcher);
	await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [
			{ id: '1', route: 'A' },
			{ id: '2', route: 'Z' }
		]
	});
	await settle();
	http.calls[0].respond(http.json({ unknown_route_ids: ['Z'] }, 404));
	await settle();
	expect(document.querySelector('[data-failed="Z"]')).not.toBeNull();
	expect(http.routes(http.calls[1])).toEqual(['A']);
	http.calls[1].respond(http.json(http.service('mta_bus', { A: ['a1'] })));
	await settle();
	expect(text('A')).toEqual(['a1']);
});

test('historical entries are separate, bounded, and protected while read', async () => {
	const http = transport();
	const data = store(http.fetcher, ['mta_bus'], 1);
	const view = await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [
			{ id: 'live', route: 'A' },
			{ id: 'h1', route: 'A', at: 100 }
		]
	});
	await settle();
	expect(http.calls.map((call) => call.url.searchParams.get('at'))).toEqual([null, '100']);
	http.calls[0].respond(http.json(http.service('mta_bus', { A: ['live'] })));
	http.calls[1].respond(http.json({ ...http.service('mta_bus', { A: ['old'] }), at: 100 }));
	await settle();
	expect(text('A')).toEqual(['live', 'old']);

	for (const at of [200, 300]) {
		await view.rerender({
			readers: [
				{ id: 'h1', route: 'A', at: 100 },
				{ id: `h${at}`, route: 'A', at }
			]
		});
		await settle();
		http.calls.at(-1)!.respond(http.json({ ...http.service('mta_bus', { A: [`t${at}`] }), at }));
		await settle();
	}
	const before = http.calls.length;
	await view.rerender({ readers: [{ id: 'h1', route: 'A', at: 100 }] });
	await settle();
	expect(http.calls).toHaveLength(before);
	await view.rerender({ readers: [] });
	await view.rerender({ readers: [{ id: 'again', route: 'A', at: 200 }] });
	await settle();
	expect(http.calls).toHaveLength(before + 1);
});

test('whole-source subway loads eagerly and serves every route from one request', async () => {
	const http = transport();
	const data = store(http.fetcher, ['mta_subway']);
	await settle();
	expect(http.calls.map((call) => call.url.pathname)).toEqual(['/api/v1/service/mta_subway']);
	http.calls[0].respond(
		http.json({ ...http.service('mta_subway', { '1': ['t1'], '7': [] }), source: 'mta_subway' })
	);
	await render(TransitDataHarness, {
		data,
		source: 'mta_subway',
		readers: [
			{ id: '1', route: '1' },
			{ id: '2', route: '7' }
		]
	});
	await settle();
	expect(text('1')).toEqual(['t1']);
	expect(text('7')).toEqual(['']);
	expect(http.calls).toHaveLength(1);
});

test('map-only owners and loaded peeks defer passenger requests until an itinerary binds', async () => {
	const http = transport();
	const data = new TransitData({
		sources: ['mta_subway'],
		eager_service: false,
		fetcher: http.fetcher,
		batch_ms: 0
	});
	created.push(data);
	await settle();
	expect(data.loadedTrip('mta_subway', '4', 't1')).toBeUndefined();
	expect(data.loadedStopArrivals('mta_subway', 'S1').calls).toEqual([]);
	expect(http.calls).toHaveLength(0);
	const view = await render(TransitDataHarness, {
		data,
		source: 'mta_subway',
		readers: [{ id: '1', route: '4' }]
	});
	await settle();
	expect(http.calls).toHaveLength(1);
	http.calls[0].respond(http.json(http.service('mta_subway', { '4': ['t1'] })));
	await settle();
	expect(text('4')).toEqual(['t1']);
	await view.unmount();
	clock += 31_000;
	document.dispatchEvent(new Event('visibilitychange'));
	await settle();
	expect(http.calls).toHaveLength(1);
	data.setEagerService(true);
	await settle();
	expect(http.calls).toHaveLength(2);
});

test('destroying the owner aborts its requests', async () => {
	const http = transport();
	const data = store(http.fetcher);
	await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [{ id: '1', route: 'A' }]
	});
	await settle();
	data.destroy();
	expect(http.calls[0].signal.aborted).toBe(true);
});

test.each(['request', 'response body'])(
	'a stalled initial service %s times out and Retry can recover',
	async (stalled) => {
		vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
		let deadline = new AbortController();
		const timeout = vi.spyOn(AbortSignal, 'timeout').mockImplementation(() => deadline.signal);
		const http = transport();
		const data = store(http.fetcher);
		await render(TransitDataHarness, {
			data,
			source: 'mta_bus',
			readers: [{ id: '1', route: 'A' }]
		});
		await tick();
		await vi.advanceTimersByTimeAsync(0);
		expect(http.calls).toHaveLength(1);
		if (stalled === 'response body') {
			http.calls[0].respond(
				new Response(
					new ReadableStream({
						start(controller) {
							http.calls[0].signal.addEventListener('abort', () =>
								controller.error(http.calls[0].signal.reason)
							);
						}
					})
				)
			);
			await vi.advanceTimersByTimeAsync(0);
		}
		expect(timeout).toHaveBeenCalledWith(15_000);
		deadline.abort(new DOMException('Request timed out', 'TimeoutError'));
		await vi.advanceTimersByTimeAsync(0);
		await tick();
		expect(http.calls[0].signal.reason.name).toBe('TimeoutError');
		expect(document.querySelector('[data-failed="A"]')).not.toBeNull();
		expect(data.status.warning).toBe(true);
		deadline = new AbortController();
		data.retry();
		await vi.advanceTimersByTimeAsync(0);
		expect(http.calls).toHaveLength(2);
		http.calls[1].respond(http.json(http.service('mta_bus', { A: ['recovered'] })));
		await vi.advanceTimersByTimeAsync(0);
		await tick();
		expect(text('A')).toEqual(['recovered']);
		expect(data.status.warning).toBe(false);
	}
);

test('a stalled refresh keeps loaded service and releases Retry after its deadline', async () => {
	vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
	let deadline = new AbortController();
	vi.spyOn(AbortSignal, 'timeout').mockImplementation(() => deadline.signal);
	const http = transport();
	const data = store(http.fetcher);
	await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [{ id: '1', route: 'A' }]
	});
	await tick();
	await vi.advanceTimersByTimeAsync(0);
	http.calls[0].respond(http.json(http.service('mta_bus', { A: ['retained'] })));
	await vi.advanceTimersByTimeAsync(0);
	await tick();
	data.retry();
	await vi.advanceTimersByTimeAsync(0);
	deadline.abort(new DOMException('Request timed out', 'TimeoutError'));
	await vi.advanceTimersByTimeAsync(0);
	await tick();
	expect(text('A')).toEqual(['retained']);
	expect(data.status.warning).toBe(true);
	deadline = new AbortController();
	data.retry();
	await vi.advanceTimersByTimeAsync(0);
	expect(http.calls).toHaveLength(3);
	http.calls[2].respond(http.json(http.service('mta_bus', { A: ['fresh'] })));
	await vi.advanceTimersByTimeAsync(0);
	await tick();
	expect(text('A')).toEqual(['fresh']);
	expect(data.status.warning).toBe(false);
});

test('a replacement catalog retries an active route previously reported unknown', async () => {
	const http = transport();
	const data = store(http.fetcher);
	await render(TransitDataHarness, { data, source: 'mta_bus', readers: [{ id: '1', route: '4' }] });
	await settle();
	http.calls[0].respond(http.json({ unknown_route_ids: ['4'] }, 404));
	await settle();
	expect(document.querySelector('[data-failed="4"]')).not.toBeNull();
	data.replaceCatalog(chartFixture('mta_bus').catalog);
	await settle();
	expect(http.calls).toHaveLength(2);
	http.calls[1].respond(http.json(http.service('mta_bus', { '4': ['recovered'] })));
	await settle();
	expect(text('4')).toEqual(['recovered']);
	expect(document.querySelector('[data-failed="4"]')).toBeNull();
});
