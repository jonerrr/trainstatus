import type { TypedVehiclePosition } from '#lib/resources/index.svelte.js';

import { describe, expect, it } from 'vitest';

import { index_positions } from './positions.svelte';

function position(vehicle_id: string, updated_at: Date): TypedVehiclePosition<'mta_subway'> {
	return {
		vehicle_id,
		trip_id: 'trip-1',
		updated_at,
		data: { source: 'mta_subway', assigned: true }
	};
}

describe('index_positions', () => {
	it('keys positions by vehicle id and converts dates', () => {
		const updated = new Date('2026-09-07T00:00:00Z');
		const result = index_positions([
			position('vehicle-1', updated),
			position('vehicle-2', updated)
		]);

		expect([...result.keys()]).toEqual(['vehicle-1', 'vehicle-2']);
		expect(result.has('trip-1')).toBe(false);
		expect(result.get('vehicle-1')?.updated_at).toBeInstanceOf(Date);
		expect(result.get('vehicle-1')?.updated_at).not.toBe(updated);
		expect(result.get('vehicle-1')?.updated_at.getTime()).toBe(updated.getTime());
	});
});
