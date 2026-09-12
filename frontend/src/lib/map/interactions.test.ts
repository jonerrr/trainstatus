import { describe, expect, it, vi } from 'vitest';

import type { ActiveVehicle } from './trajectoryArrow';
import { MapInteractionController, type MapTarget, resolveMapTargets } from './interactions';

const trip: MapTarget = {
	kind: 'trip',
	id: 'trip-1',
	source: 'mta_subway',
	label: 'A train'
};

const route: MapTarget = {
	kind: 'route',
	id: 'A',
	source: 'mta_subway',
	label: 'A · Eighth Avenue Express'
};

function mockVehicle(overrides: Partial<ActiveVehicle> = {}): ActiveVehicle {
	return {
		renderUnitId: 'unit-1',
		source: 'mta_subway',
		tripId: 'trip-1',
		routeId: 'A',
		iconKey: 'subway',
		unitIndex: 0,
		unitCount: 1,
		isHead: true,
		lengthM: 20,
		passengers: null,
		color: [0, 0, 0, 255],
		position: [0, 0],
		bearing: 0,
		...overrides
	};
}

describe('resolveMapTargets', () => {
	it('opens a single unique target directly', () => {
		expect(resolveMapTargets([trip, { ...trip }])).toEqual({ kind: 'open', target: trip });
	});

	it('offers a chooser for overlapping targets in visual priority order', () => {
		expect(resolveMapTargets([route, trip])).toEqual({
			kind: 'choose',
			targets: [trip, route]
		});
	});

	it('does nothing when no target was picked', () => {
		expect(resolveMapTargets([])).toEqual({ kind: 'none' });
	});

	it('combines MapLibre targets with deduplicated deck vehicle picks', () => {
		const controller = new MapInteractionController();
		controller.registerVehiclePicker({
			pick: vi.fn(() => [mockVehicle(), mockVehicle()])
		});

		expect(controller.resolve({ x: 20, y: 30 }, [route])).toEqual({
			kind: 'choose',
			targets: [
				{
					kind: 'trip',
					id: 'trip-1',
					source: 'mta_subway',
					label: 'A vehicle',
					subtitle: 'trip-1'
				},
				route
			]
		});
	});
});
