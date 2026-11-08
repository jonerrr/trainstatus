import { describe, expect, it, vi } from 'vitest';

import { MapInteractionController, resolveMapTargets, type MapTarget } from './interactions';
import type { ActiveVehicle } from './trajectory';

const trip: MapTarget = {
	kind: 'trip',
	id: 'trip-1',
	source: 'mta_subway',
	routeId: 'A'
};

const route: MapTarget = {
	kind: 'route',
	id: 'A',
	source: 'mta_subway'
};

function mockVehicle(overrides: Partial<ActiveVehicle> = {}): ActiveVehicle {
	return {
		renderUnitId: 'unit-1',
		source: 'mta_subway',
		tripId: 'trip-1',
		routeId: 'A',
		iconKey: 'subway',
		isHead: true,
		lengthM: 20,
		color: [0, 0, 0],
		position: [0, 0],
		bearing: 0,
		...overrides
	};
}

describe('resolveMapTargets', () => {
	it('opens a single unique target directly', () => {
		expect(resolveMapTargets([trip, { ...trip }])).toEqual([trip]);
	});

	it('offers a chooser for overlapping targets in visual priority order', () => {
		expect(resolveMapTargets([route, trip])).toEqual([trip, route]);
	});

	it('does nothing when no target was picked', () => {
		expect(resolveMapTargets([])).toEqual([]);
	});

	it('combines MapLibre targets with deduplicated deck vehicle picks', () => {
		const controller = new MapInteractionController();
		controller.registerVehiclePicker({
			pickClosest: vi.fn(() => mockVehicle()),
			pick: vi.fn(() => [mockVehicle(), mockVehicle()])
		});

		expect(controller.resolve({ x: 20, y: 30 }, [route])).toEqual([
			{
				kind: 'trip',
				id: 'trip-1',
				source: 'mta_subway',
				routeId: 'A'
			},
			route
		]);
	});

	it('uses one closest-object query for hover and the overlap query for clicks', () => {
		const controller = new MapInteractionController();
		const pickClosest = vi.fn(() => mockVehicle());
		const pick = vi.fn(() => [mockVehicle(), mockVehicle({ tripId: 'trip-2' })]);
		controller.registerVehiclePicker({ pickClosest, pick });
		expect(controller.resolve({ x: 20, y: 30 }, [route], true)).toEqual([trip, route]);
		expect(pickClosest).toHaveBeenCalledTimes(1);
		expect(pick).not.toHaveBeenCalled();
		expect(controller.resolve({ x: 20, y: 30 }, [route]).map((target) => target.id)).toEqual([
			'trip-1',
			'trip-2',
			'A'
		]);
		expect(pick).toHaveBeenCalledTimes(1);
	});
});
