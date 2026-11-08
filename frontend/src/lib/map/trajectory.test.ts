import type { TrajectoryShape } from '#lib/client/index.js';

import { expect, test } from 'vitest';

import vectors from '../../../../backend/tests/fixtures/trajectory/motion-vectors.json';
import {
	buildActiveVehiclesAtTime,
	evaluateMotion,
	positionAtDistance,
	renderUnitTableFromIPC
} from './trajectory';
import { trajectoryFixture } from './trajectoryFixture';

test('decodes shared geometry and renders expected positions through a corner', () => {
	const table = renderUnitTableFromIPC(trajectoryFixture());
	for (const [time, x, y] of [
		[100, -74, 40],
		[125, -73.6875, 40],
		[150, -73, 40],
		[175, -73, 40.6875],
		[200, -73, 41]
	]) {
		expect(buildActiveVehiclesAtTime(table, time)[0].position).toEqual([x, y]);
	}
	expect(buildActiveVehiclesAtTime(table, 100)[0].bearing).toBeCloseTo(90, 1);
	expect(buildActiveVehiclesAtTime(table, 200)[0].bearing).toBeCloseTo(0, 1);
});
test('reuses pooled vehicles and removes absent rows', () => {
	const table = renderUnitTableFromIPC(trajectoryFixture(['one', 'two']));
	const vehicles = buildActiveVehiclesAtTime(table, 100),
		first = vehicles[0];
	buildActiveVehiclesAtTime(table, 200, vehicles);
	expect(vehicles[0]).toBe(first);
	expect(first.position).toEqual([-73, 41]);
	buildActiveVehiclesAtTime(renderUnitTableFromIPC(trajectoryFixture(['one'])), 150, vehicles);
	expect(vehicles).toHaveLength(1);
});
test('car offsets traverse different segments at the corner with shared head motion', () => {
	const table = renderUnitTableFromIPC(trajectoryFixture(['head', 'car']));
	table.units[1].offset = 10;
	const vehicles = buildActiveVehiclesAtTime(table, 150);
	expect(vehicles[0].position).toEqual([-73, 40]);
	expect(vehicles[1].position[0]).toBeCloseTo(-73.2);
	expect(vehicles[1].position[1]).toBe(40);
	expect(table.units[0].trip).toBe(table.units[1].trip);
});
test('lookup handles repeated coordinates and zero-length geometry edges in backend meters', () => {
	const shape: TrajectoryShape = {
		key: 'loop',
		positions: [
			[0, 0],
			[1, 0],
			[1, 0],
			[1, 1],
			[0, 0],
			[0, 1]
		],
		distances_m: [0, 10, 10, 20, 30, 40]
	};
	expect(positionAtDistance(shape, 5)).toEqual([0.5, 0]);
	expect(positionAtDistance(shape, 10)).toEqual([1, 0]);
	expect(positionAtDistance(shape, 35)).toEqual([0, 0.5]);
	expect(positionAtDistance(shape, -1)).toEqual([0, 0]);
	expect(positionAtDistance(shape, 50)).toEqual([0, 1]);
});

test('backend and browser share numerical vectors including joins and holds', () => {
	for (const [time, distance, speed] of vectors.samples) {
		const state = evaluateMotion(vectors.motion, time);
		expect(Math.abs(state.distance - distance)).toBeLessThan(vectors.tolerance);
		expect(Math.abs(state.speed - speed)).toBeLessThan(vectors.tolerance);
	}
});
