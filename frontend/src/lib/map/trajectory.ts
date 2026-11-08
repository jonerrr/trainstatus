import type { MotionInterval, Source, TrajectoryShape } from '#lib/client/index.js';

import { tableFromIPC, type Vector } from 'apache-arrow';

interface TripMotion {
	distance: number;
	shape: TrajectoryShape;
	motion: MotionInterval[];
}
interface Unit extends ActiveVehicle {
	offset: number;
	trip: TripMotion;
}
export interface RenderUnitTable {
	units: Unit[];
	trips: Map<string, TripMotion>;
}
export interface ActiveVehicle {
	renderUnitId: string;
	source: Source;
	tripId: string;
	routeId: string;
	iconKey: string;
	isHead: boolean;
	lengthM: number;
	color: [number, number, number];
	position: [number, number];
	bearing: number;
}
function list(value: Vector): number[] {
	return Array.from({ length: value.length }, (_, i) => Number(value.get(i)));
}
function pair(value: Vector): [number, number] {
	return [Number(value.get(0)), Number(value.get(1))];
}

export function renderUnitTableFromIPC(buffer: ArrayBuffer): RenderUnitTable {
	const table = tableFromIPC(new Uint8Array(buffer));
	const get = (name: string, row: number) => table.getChild(name)!.get(row);
	const shapes = new Map<string, TrajectoryShape>();
	const trips = new Map<string, TripMotion>();
	for (let row = 0; row < table.numRows; row++) {
		const key = String(get('shape_key', row));
		const positions = get('shape_positions', row) as Vector;
		if (positions.length) {
			shapes.set(key, {
				key,
				positions: Array.from({ length: positions.length }, (_, i) => pair(positions.get(i))),
				distances_m: list(get('shape_distances', row))
			});
		}
	}
	for (let row = 0; row < table.numRows; row++) {
		const values = get('motion', row) as Vector;
		if (!values.length) continue;
		const motion = Array.from({ length: values.length }, (_, i) => {
			const v = list(values.get(i));
			return { t0: v[0], t1: v[1], a: v[2], b: v[3], c: v[4], d: v[5] };
		});
		const shape = shapes.get(String(get('shape_key', row)))!;
		const source = get('source', row) as Source;
		const key = `${source}:${get('trip_id', row)}`;
		trips.set(key, { distance: 0, shape, motion });
	}
	const units: Unit[] = [];
	for (let row = 0; row < table.numRows; row++) {
		const source = get('source', row) as Source;
		const tripId = String(get('trip_id', row));
		const trip = trips.get(`${source}:${tripId}`)!;
		const color = list(get('color', row));
		units.push({
			renderUnitId: String(get('render_unit_id', row)),
			source,
			tripId,
			routeId: String(get('route_id', row)),
			iconKey: String(get('icon_key', row)),
			isHead: Boolean(get('is_head', row)),
			lengthM: Number(get('length_m', row)),
			color: [color[0], color[1], color[2]],
			position: [0, 0],
			bearing: 0,
			offset: Number(get('offset_m', row)),
			trip
		});
	}
	return { units, trips };
}

/** Binary search uses exactly the backend's projected distance metric. */
export function positionAtDistance(shape: TrajectoryShape, distance: number): [number, number] {
	const d = shape.distances_m,
		p = shape.positions;
	const s = Math.max(0, Math.min(distance, d[d.length - 1]));
	let lo = 0,
		hi = d.length - 1;
	while (lo < hi) {
		const mid = (lo + hi) >>> 1;
		if (d[mid] <= s) lo = mid + 1;
		else hi = mid;
	}
	const i = Math.max(0, lo - 1),
		j = Math.min(i + 1, d.length - 1);
	const u = d[j] === d[i] ? 0 : (s - d[i]) / (d[j] - d[i]);
	return [p[i][0] + u * (p[j][0] - p[i][0]), p[i][1] + u * (p[j][1] - p[i][1])];
}
function bearing(shape: TrajectoryShape, s: number): number {
	const from = positionAtDistance(shape, s - 2),
		to = positionAtDistance(shape, s + 2);
	const lat1 = (from[1] * Math.PI) / 180,
		lat2 = (to[1] * Math.PI) / 180,
		dlon = ((to[0] - from[0]) * Math.PI) / 180;
	return (
		((Math.atan2(
			Math.sin(dlon) * Math.cos(lat2),
			Math.cos(lat1) * Math.sin(lat2) - Math.sin(lat1) * Math.cos(lat2) * Math.cos(dlon)
		) *
			180) /
			Math.PI +
			360) %
		360
	);
}
export function buildActiveVehiclesAtTime(
	renderUnits: RenderUnitTable,
	t: number,
	target: ActiveVehicle[] = []
): ActiveVehicle[] {
	for (const trip of renderUnits.trips.values())
		trip.distance = evaluateMotion(trip.motion, t).distance;
	let count = 0;
	for (const unit of renderUnits.units) {
		const distance = unit.trip.distance - unit.offset;
		if (
			unit.source === 'mta_subway' &&
			(distance < 0 || distance > unit.trip.shape.distances_m.at(-1)!)
		)
			continue;
		const position = positionAtDistance(unit.trip.shape, distance);
		unit.position[0] = position[0];
		unit.position[1] = position[1];
		unit.bearing = bearing(unit.trip.shape, distance);
		target[count++] = unit;
	}
	target.length = count;
	return target;
}
export function normalizeBearingForIcon(bearing: number): number {
	return (360 - bearing) % 360;
}

export interface MotionState {
	distance: number;
	speed: number;
}

export function evaluateMotion(motion: readonly MotionInterval[], t: number): MotionState {
	let lo = 0,
		hi = motion.length;
	while (lo < hi) {
		const mid = (lo + hi) >>> 1;
		if (motion[mid].t1 <= t) lo = mid + 1;
		else hi = mid;
	}
	const p = motion[Math.min(lo, motion.length - 1)];
	const u = Math.max(0, Math.min(1, (t - p.t0) / (p.t1 - p.t0)));
	return {
		distance: p.a + u * (p.b + u * (p.c + u * p.d)),
		speed:
			t <= motion[0].t0 || t >= motion[motion.length - 1].t1
				? 0
				: (p.b + u * (2 * p.c + 3 * p.d * u)) / (p.t1 - p.t0)
	};
}
