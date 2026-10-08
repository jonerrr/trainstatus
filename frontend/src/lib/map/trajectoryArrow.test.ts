import { expect, test } from 'vitest';

import { buildActiveVehiclesAtTime, renderUnitTableFromIPC } from './trajectoryArrow';
import { trajectoryFixture } from './trajectoryFixture';

test('interpolates coordinates and takes the short bearing path through north', () => {
	const table = renderUnitTableFromIPC(trajectoryFixture());
	const vehicles = buildActiveVehiclesAtTime(table, 150);
	expect(vehicles[0].position).toEqual([-73.5, 40.5]);
	expect(vehicles[0].bearing).toBe(0);
	expect(vehicles[0].tripId).toBe('trip-1');
});
test('reuses pooled vehicles and removes rows absent from the next snapshot', () => {
	const table = renderUnitTableFromIPC(trajectoryFixture(['one', 'two']));
	const vehicles = buildActiveVehiclesAtTime(table, 100);
	const first = vehicles[0];
	buildActiveVehiclesAtTime(table, 200, vehicles);
	expect(vehicles[0]).toBe(first);
	expect(first.position).toEqual([-73, 41]);
	buildActiveVehiclesAtTime(renderUnitTableFromIPC(trajectoryFixture(['one'])), 150, vehicles);
	expect(vehicles).toHaveLength(1);
});
