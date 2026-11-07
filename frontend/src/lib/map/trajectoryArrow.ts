import type { GpsEvidence, MotionInterval, Source, TrajectoryShape } from '#lib/client/index.js';

import { tableFromIPC, type Vector } from 'apache-arrow';

import { evaluateMotion, MotionControllers, validateMotion } from './trajectoryMotion';

interface TripMotion {
	shape: TrajectoryShape;
	motion: MotionInterval[];
	evidence: GpsEvidence | null;
}
interface Unit {
	renderUnitId: string;
	source: Source;
	tripId: string;
	routeId: string;
	iconKey: string;
	unitIndex: number | null;
	unitCount: number | null;
	isHead: boolean;
	lengthM: number;
	passengers: number | null;
	color: [number, number, number, number];
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
	unitIndex: number | null;
	unitCount: number | null;
	isHead: boolean;
	lengthM: number;
	passengers: number | null;
	color: [number, number, number, number];
	position: [number, number];
	bearing: number;
}
function list(value: Vector): number[] {
	return Array.from({ length: value.length }, (_, i) => Number(value.get(i)));
}

export function renderUnitTableFromIPC(buffer: ArrayBuffer): RenderUnitTable {
	const table = tableFromIPC(new Uint8Array(buffer));
	const names = [
		'render_unit_id',
		'source',
		'trip_id',
		'route_id',
		'icon_key',
		'unit_index',
		'unit_count',
		'is_head',
		'length_m',
		'passengers',
		'color',
		'shape_key',
		'offset_m',
		'shape_positions',
		'shape_distances',
		'motion',
		'observation_vehicle',
		'observed_at',
		'observed_distance',
		'correction_deadline'
	];
	for (const name of names)
		if (!table.getChild(name)) throw new Error(`Trajectories Arrow schema missing ${name}`);
	const get = (name: string, row: number) => table.getChild(name)!.get(row);
	const shapes = new Map<string, TrajectoryShape>();
	const trips = new Map<string, TripMotion>();
	for (let row = 0; row < table.numRows; row++) {
		const key = String(get('shape_key', row));
		const positions = get('shape_positions', row) as Vector;
		if (positions.length) {
			const points = Array.from({ length: positions.length }, (_, i) => list(positions.get(i)));
			const distances = list(get('shape_distances', row));
			if (
				points.length < 2 ||
				distances.length !== points.length ||
				points.some((p) => p.length !== 2 || !p.every(Number.isFinite)) ||
				distances[0] !== 0 ||
				distances.some((v, i) => !Number.isFinite(v) || (i > 0 && v < distances[i - 1])) ||
				distances.at(-1)! <= 0 ||
				shapes.has(key)
			)
				throw new Error('Invalid shared trajectory shape');
			shapes.set(key, { key, positions: points, distances_m: distances });
		}
	}
	for (let row = 0; row < table.numRows; row++) {
		const values = get('motion', row) as Vector;
		if (!values.length) continue;
		const motion = Array.from({ length: values.length }, (_, i) => {
			const v = list(values.get(i));
			if (v.length !== 6) throw new Error('Invalid motion coefficient count');
			return { t0: v[0], t1: v[1], a: v[2], b: v[3], c: v[4], d: v[5] };
		});
		validateMotion(motion);
		const shape = shapes.get(String(get('shape_key', row)));
		if (!shape) throw new Error('Missing trajectory shape');
		const observed = get('observed_at', row);
		const evidence =
			observed == null
				? null
				: {
						vehicle_id: String(get('observation_vehicle', row)),
						observed_at: Number(observed),
						distance_m: Number(get('observed_distance', row)),
						correction_deadline: Number(get('correction_deadline', row))
					};
		if (
			evidence &&
			![evidence.observed_at, evidence.distance_m, evidence.correction_deadline].every(
				Number.isFinite
			)
		)
			throw new Error('Invalid GPS evidence');
		const key = `${get('source', row)}:${get('trip_id', row)}`;
		if (trips.has(key)) throw new Error('Duplicate trajectory plan');
		trips.set(key, { shape, motion, evidence });
	}
	const units: Unit[] = [];
	for (let row = 0; row < table.numRows; row++) {
		const source = get('source', row) as Source;
		if (!['mta_subway', 'mta_bus', 'njt_bus'].includes(source))
			throw new Error('Invalid trajectory source');
		const tripId = String(get('trip_id', row));
		const trip = trips.get(`${source}:${tripId}`);
		if (!trip || trip.shape.key !== get('shape_key', row))
			throw new Error('Missing trajectory plan');
		const offset = Number(get('offset_m', row));
		if (!Number.isFinite(offset) || offset < 0) throw new Error('Invalid car offset');
		const color = list(get('color', row));
		units.push({
			renderUnitId: String(get('render_unit_id', row)),
			source,
			tripId,
			routeId: String(get('route_id', row)),
			iconKey: String(get('icon_key', row)),
			unitIndex: get('unit_index', row),
			unitCount: get('unit_count', row),
			isHead: Boolean(get('is_head', row)),
			lengthM: Number(get('length_m', row)),
			passengers: get('passengers', row),
			color: [color[0], color[1], color[2], 235],
			offset,
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
	target: ActiveVehicle[] = [],
	controllers?: MotionControllers
): ActiveVehicle[] {
	const heads = new Map<string, number>();
	for (const [key, trip] of renderUnits.trips)
		heads.set(
			key,
			controllers
				? controllers.sample(
						key,
						trip.shape.key,
						trip.motion,
						trip.evidence,
						t,
						!key.startsWith('mta_subway:')
					).distance
				: evaluateMotion(trip.motion, t).distance
		);
	controllers?.retain(new Set(renderUnits.trips.keys()));
	let count = 0;
	for (const unit of renderUnits.units) {
		const distance = heads.get(`${unit.source}:${unit.tripId}`)! - unit.offset;
		if (
			unit.source === 'mta_subway' &&
			(distance < 0 || distance > unit.trip.shape.distances_m.at(-1)!)
		)
			continue;
		const position = positionAtDistance(unit.trip.shape, distance);
		const vehicle = target[count] ?? ({ position: [0, 0], color: [0, 0, 0, 235] } as ActiveVehicle);
		Object.assign(vehicle, {
			renderUnitId: unit.renderUnitId,
			source: unit.source,
			tripId: unit.tripId,
			routeId: unit.routeId,
			iconKey: unit.iconKey,
			unitIndex: unit.unitIndex,
			unitCount: unit.unitCount,
			isHead: unit.isHead,
			lengthM: unit.lengthM,
			passengers: unit.passengers
		});
		vehicle.color[0] = unit.color[0];
		vehicle.color[1] = unit.color[1];
		vehicle.color[2] = unit.color[2];
		vehicle.position[0] = position[0];
		vehicle.position[1] = position[1];
		vehicle.bearing = bearing(unit.trip.shape, distance);
		target[count++] = vehicle;
	}
	target.length = count;
	return target;
}
export function normalizeBearingForIcon(bearing: number): number {
	return Number.isFinite(bearing) ? (((360 - bearing) % 360) + 360) % 360 : 0;
}
