import type { ServiceTrip } from '#lib/client/index.js';

import { describe, expect, it } from 'vitest';

import { stopArrivals } from './arrivals.js';
import { type Call, type RouteData } from './decode.js';

const now = Date.parse('2026-10-05T12:00:00Z');

function route(routeId: string, trips: [string, number][]): RouteData {
	return {
		source: 'mta_subway',
		routeId,
		at: undefined,
		fetchedAt: null,
		callsByTrip: new Map(),
		callsByStop: new Map(),
		trips: new Map(
			trips.map(([id, direction]): [string, ServiceTrip] => [
				id,
				{
					id,
					vehicle_id: id,
					direction,
					data: { source: 'mta_subway', consist_cars: [] },
					status: null
				}
			])
		)
	};
}

function call(routeId: string, tripId: string, minutes: number): Call {
	return {
		platformEdges: [],
		tripId,
		routeId,
		stopId: 'test-stop',
		sequence: minutes,
		arrival: now + minutes * 60_000,
		departure: now + minutes * 60_000
	};
}

describe('stopArrivals', () => {
	const routes = [
		route('7', [['local', 1]]),
		route('7X', [['express', 3]]),
		route('6X', [['past', 1]])
	];
	const calls = [
		call('6X', 'past', -1),
		call('7', 'missing', 1),
		call('7', 'local', 0),
		call('7X', 'express', 2),
		call('7X', 'express', 3)
	];

	it('resolves upcoming arrivals and derives routes only from matching trips', () => {
		const result = stopArrivals(calls, routes, now);
		expect(result.arrivals.map((a) => [a.trip.id, a.trip.direction, a.eta])).toEqual([
			['local', 1, 0],
			['express', 3, 2],
			['express', 3, 3]
		]);
		expect([...result.activeRoutes]).toEqual(['7', '7X']);
		expect(calls).toHaveLength(5);
	});

	it('includes previous arrivals when requested but still skips unresolved trips', () => {
		const result = stopArrivals(calls, routes, now, { showPrevious: true });
		expect(result.arrivals.map((a) => a.eta)).toEqual([-1, 0, 2, 3]);
		expect([...result.activeRoutes]).toEqual(['6X', '7', '7X']);
	});

	it('supports the modal cutoff that excludes arrivals exactly at the current time', () => {
		const result = stopArrivals(calls, routes, now, { includeDueNow: false });
		expect(result.arrivals.map((a) => a.eta)).toEqual([2, 3]);
		expect([...result.activeRoutes]).toEqual(['7X']);
	});

	it('clears express service when arrivals expire or route data disappears', () => {
		expect([...stopArrivals(calls, routes, now + 4 * 60_000).activeRoutes]).toEqual([]);
		expect(stopArrivals(calls, [], now).arrivals).toEqual([]);
		expect([...stopArrivals([], routes, now).activeRoutes]).toEqual([]);
	});
});
