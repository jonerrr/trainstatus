import type { ServiceTrip } from '#lib/client/index.js';

import { describe, expect, it } from 'vitest';

import { stop_arrivals } from './arrivals.js';
import { type Call, type RouteData } from './decode.js';

const now = Date.parse('2026-10-05T12:00:00Z');

function route(route_id: string, trips: [string, number][]): RouteData {
	return {
		source: 'mta_subway',
		route_id,
		at: undefined,
		fetched_at: null,
		calls_by_trip: new Map(),
		calls_by_stop: new Map(),
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

function call(route_id: string, trip_id: string, minutes: number): Call {
	return {
		platform_edges: [],
		trip_id,
		route_id,
		stop_id: 'test-stop',
		sequence: minutes,
		arrival: now + minutes * 60_000,
		departure: now + minutes * 60_000
	};
}

describe('stop_arrivals', () => {
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
		const result = stop_arrivals(calls, routes, now);
		expect(result.arrivals.map((a) => [a.trip.id, a.trip.direction, a.eta])).toEqual([
			['local', 1, 0],
			['express', 3, 2],
			['express', 3, 3]
		]);
		expect([...result.active_routes]).toEqual(['7', '7X']);
		expect(calls).toHaveLength(5);
	});

	it('includes previous arrivals when requested but still skips unresolved trips', () => {
		const result = stop_arrivals(calls, routes, now, { show_previous: true });
		expect(result.arrivals.map((a) => a.eta)).toEqual([-1, 0, 2, 3]);
		expect([...result.active_routes]).toEqual(['6X', '7', '7X']);
	});

	it('supports the modal cutoff that excludes arrivals exactly at the current time', () => {
		const result = stop_arrivals(calls, routes, now, { include_due_now: false });
		expect(result.arrivals.map((a) => a.eta)).toEqual([2, 3]);
		expect([...result.active_routes]).toEqual(['7X']);
	});

	it('clears express service when arrivals expire or route data disappears', () => {
		expect([...stop_arrivals(calls, routes, now + 4 * 60_000).active_routes]).toEqual([]);
		expect(stop_arrivals(calls, [], now).arrivals).toEqual([]);
		expect([...stop_arrivals([], routes, now).active_routes]).toEqual([]);
	});
});
