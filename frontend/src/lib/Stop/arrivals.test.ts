import type { StopTime, Trip } from '#lib/client/index.js';

import { describe, expect, it } from 'vitest';

import { get_stop_arrivals } from './arrivals.js';

const now = Date.parse('2026-10-05T12:00:00Z');

function trip(id: string, route_id: string, direction = 1): Trip {
	return {
		id,
		original_id: id,
		route_id,
		direction,
		vehicle_id: id,
		shape_ids: [],
		created_at: new Date(now),
		updated_at: new Date(now),
		data: { source: 'mta_subway', consist_cars: [] }
	};
}

function stop_time(trip_id: string, minutes: number): StopTime {
	return {
		trip_id,
		stop_id: 'test-stop',
		arrival: new Date(now + minutes * 60_000),
		departure: new Date(now + minutes * 60_000),
		data: { source: 'mta_subway' }
	};
}

describe('get_stop_arrivals', () => {
	const trips = new Map([
		['local', trip('local', '7')],
		['express', trip('express', '7X', 3)],
		['past', trip('past', '6X')]
	]);
	const times = [
		stop_time('past', -1),
		stop_time('missing', 1),
		stop_time('local', 0),
		stop_time('express', 2),
		stop_time('express', 3)
	];

	it('resolves upcoming arrivals and derives routes only from matching trips', () => {
		const result = get_stop_arrivals(times, trips, now);
		expect(result.arrivals.map((st) => [st.trip.id, st.trip.direction, st.eta])).toEqual([
			['local', 1, 0],
			['express', 3, 2],
			['express', 3, 3]
		]);
		expect([...result.active_routes]).toEqual(['7', '7X']);
		expect(times).toHaveLength(5);
	});

	it('includes previous arrivals when requested but still skips unresolved trips', () => {
		const result = get_stop_arrivals(times, trips, now, { show_previous: true });
		expect(result.arrivals.map((st) => st.eta)).toEqual([-1, 0, 2, 3]);
		expect([...result.active_routes]).toEqual(['6X', '7', '7X']);
	});

	it('supports the modal cutoff that excludes arrivals exactly at the current time', () => {
		const result = get_stop_arrivals(times, trips, now, { include_due_now: false });
		expect(result.arrivals.map((st) => st.eta)).toEqual([2, 3]);
		expect([...result.active_routes]).toEqual(['7X']);
	});

	it('clears express service when arrivals expire or trip data disappears', () => {
		expect([...get_stop_arrivals(times, trips, now + 4 * 60_000).active_routes]).toEqual([]);
		expect(get_stop_arrivals(times, undefined, now).arrivals).toEqual([]);
		expect([...get_stop_arrivals([], trips, now).active_routes]).toEqual([]);
	});
});
