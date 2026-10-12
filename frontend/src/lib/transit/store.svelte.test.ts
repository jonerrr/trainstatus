import { tick } from 'svelte';

import type { RouteService, Service, Source } from '#lib/client/index.js';
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
	const data = new TransitData({ sources, fetcher, history_limit, batch_ms: 0, now: () => clock });
	created.push(data);
	return data;
}

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
	vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true);
});

afterEach(() => {
	vi.restoreAllMocks();
	for (const data of created.splice(0)) data.destroy();
	clock = 1_780_000_000_000;
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
	expect(data.status.last_fetched).toBe(clock);
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
	const fetched = data.status.last_fetched;

	clock += 31_000;
	document.dispatchEvent(new Event('visibilitychange'));
	await settle();
	http.calls[1].respond(new Response('down', { status: 500 }));
	await settle();
	expect(text('A')).toEqual(['a1']);
	expect(data.status.failing).toBe(true);
	expect(data.status.stale).toBe(false);
	expect(data.status.last_fetched).toBe(fetched);

	clock += 60_000;
	document.dispatchEvent(new Event('visibilitychange'));
	await settle();
	expect(data.status.stale).toBe(true);
	expect(text('A')).toEqual(['a1']);
	http.calls[2].respond(http.json(http.service('mta_bus', { A: [] })));
	await settle();
	expect(text('A')).toEqual(['']);
	expect(data.status.failing).toBe(false);
	expect(data.status.stale).toBe(false);
});

test('a failed first load rejects into the failure UI', async () => {
	const http = transport();
	const data = store(http.fetcher);
	await render(TransitDataHarness, {
		data,
		source: 'mta_bus',
		readers: [{ id: '1', route: 'A' }]
	});
	await settle();
	http.calls[0].respond(new Response('down', { status: 503 }));
	await settle();
	expect(document.querySelector('[data-failed="A"]')?.textContent).toBe('Service returned 503');
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
		http.json({ ...http.service('mta_subway', { '1': ['t1'] }), source: 'mta_subway' })
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
