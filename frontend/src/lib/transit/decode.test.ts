import type { RouteService } from '#lib/client/index.js';

import { expect, test } from 'vitest';

import { callsAtStop, decodeRoute, decodeService, isStale, upcomingCalls } from './decode.js';

const route: RouteService = {
	trips: [
		{ id: 't1', vehicle_id: 'v1', direction: 0, data: { source: 'mta_bus' }, status: null },
		{ id: 't2', vehicle_id: 'v2', direction: 1, data: { source: 'mta_bus' }, status: null }
	],
	stops: ['loop', 'mid'],
	calls: {
		trip: [0, 0, 0, 1],
		stop: [0, 1, 0, 0],
		sequence: [1, 2, 3, 1],
		arrival: [100, 200, 300, 50],
		departure: [110, 210, 310, 60]
	}
};

test('keeps repeated visits in sequence and indexes stops by arrival', () => {
	const decoded = decodeRoute('mta_bus', 'A', route);
	expect(decoded.calls_by_trip.get('t1')?.map((call) => [call.stop_id, call.sequence])).toEqual([
		['loop', 1],
		['mid', 2],
		['loop', 3]
	]);
	expect(decoded.calls_by_stop.get('loop')?.map((call) => [call.trip_id, call.arrival])).toEqual([
		['t2', 50_000],
		['t1', 100_000],
		['t1', 300_000]
	]);
	expect(decoded.calls_by_trip.get('t2')?.[0].departure).toBe(60_000);
});

test('merges routes at a stop in arrival order', () => {
	const a = decodeRoute('mta_bus', 'A', route);
	const b = decodeRoute('mta_bus', 'B', {
		...route,
		trips: [{ ...route.trips[0], id: 'b1' }],
		calls: { trip: [0], stop: [0], sequence: [1], arrival: [75], departure: [80] }
	});
	expect(callsAtStop([a, b], 'loop').map((call) => call.trip_id)).toEqual(['t2', 'b1', 't1', 't1']);
});

test('drops passed calls but retains future predictions and departures', () => {
	const calls = decodeRoute('mta_bus', 'A', route).calls_by_stop.get('loop')!;
	expect(upcomingCalls(calls, 105_000).map((call) => call.arrival)).toEqual([100_000, 300_000]);
	expect(upcomingCalls(calls, 400_000)).toEqual([]);
	expect(upcomingCalls([], 400_000)).toEqual([]);
});

test('only live data becomes stale, after 90 seconds', () => {
	const live = decodeRoute('mta_bus', 'A', route, null, 1_000);
	expect(isStale(live, 91_000)).toBe(false);
	expect(isStale(live, 91_001)).toBe(true);
	expect(isStale(decodeRoute('mta_bus', 'A', route, 50, 1_000), 1_000_000)).toBe(false);
});

test('rejects unequal columns and out-of-range indexes', () => {
	expect(() =>
		decodeRoute('mta_bus', 'A', { ...route, calls: { ...route.calls, arrival: [1] } })
	).toThrow(/differ in length/);
	expect(() =>
		decodeRoute('mta_bus', 'A', { ...route, calls: { ...route.calls, stop: [0, 1, 0, 2] } })
	).toThrow(/stop index 2/);
	expect(() =>
		decodeRoute('mta_bus', 'A', { ...route, calls: { ...route.calls, trip: [0, 0, 0, 2] } })
	).toThrow(/trip index 2/);
});

test('fetching an old committed snapshot does not renew source freshness', () => {
	const service = { source: 'mta_bus' as const, updated_at: 2, routes: { A: route } };
	const decoded = decodeService(service, 92_000).get('A')!;
	expect(decoded.source_updated_at).toBe(2_000);
	expect(isStale(decoded, 92_001)).toBe(true);
	const historical = decodeService({ ...service, at: 50 }, 92_000).get('A')!;
	expect(historical.source_updated_at).toBeNull();
	expect(isStale(historical, 1_000_000)).toBe(false);
});
