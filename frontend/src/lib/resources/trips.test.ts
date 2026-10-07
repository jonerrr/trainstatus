import type { TypedTrip } from '#lib/resources/index.svelte.js';

import { describe, expect, it } from 'vitest';

import { index_trips } from './trips.svelte';

function trip(id: string, created_at: Date): TypedTrip<'mta_subway'> {
	return {
		id,
		original_id: id,
		route_id: '4',
		direction: 1,
		vehicle_id: `vehicle-${id}`,
		shape_ids: [],
		created_at,
		updated_at: created_at,
		data: { source: 'mta_subway', consist_cars: [] }
	};
}

describe('index_trips', () => {
	it('keys trips by id and converts dates', () => {
		const created = new Date('2026-09-07T00:00:00Z');
		const updated = new Date('2026-09-07T01:00:00Z');
		const input = trip('trip-1', created);
		input.updated_at = updated;
		const result = index_trips([input, trip('trip-2', created)]);

		expect([...result.keys()]).toEqual(['trip-1', 'trip-2']);
		expect(result.get('trip-1')?.created_at).toBeInstanceOf(Date);
		expect(result.get('trip-1')?.created_at).not.toBe(created);
		expect(result.get('trip-1')?.created_at.getTime()).toBe(created.getTime());
		expect(result.get('trip-1')?.updated_at.getTime()).toBe(updated.getTime());
	});
});
