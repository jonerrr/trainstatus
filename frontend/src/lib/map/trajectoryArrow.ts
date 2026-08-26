import { type Table, type Vector, tableFromIPC } from 'apache-arrow';

export interface RenderUnitTable {
	table: Table;
	renderUnitId: Vector;
	source: Vector;
	tripId: Vector;
	routeId: Vector;
	iconKey: Vector;
	unitIndex: Vector;
	unitCount: Vector;
	isHead: Vector;
	lengthM: Vector;
	passengers: Vector;
	color: Vector;
	timestamps: Vector;
	positions: Vector;
	bearings: Vector;
}

export interface ActiveVehicle {
	renderUnitId: string;
	source: string;
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

function isVector(value: unknown): value is Vector {
	return value != null && typeof (value as Vector).get === 'function';
}

export function renderUnitTableFromIPC(buffer: ArrayBuffer): RenderUnitTable {
	const table = tableFromIPC(new Uint8Array(buffer));

	const renderUnitId = table.getChild('render_unit_id');
	const source = table.getChild('source');
	const tripId = table.getChild('trip_id');
	const routeId = table.getChild('route_id');
	const iconKey = table.getChild('icon_key');
	const unitIndex = table.getChild('unit_index');
	const unitCount = table.getChild('unit_count');
	const isHead = table.getChild('is_head');
	const lengthM = table.getChild('length_m');
	const passengers = table.getChild('passengers');
	const color = table.getChild('color');
	const timestamps = table.getChild('timestamps');
	const positions = table.getChild('positions');
	const bearings = table.getChild('bearings');

	if (
		!renderUnitId ||
		!source ||
		!tripId ||
		!routeId ||
		!iconKey ||
		!unitIndex ||
		!unitCount ||
		!isHead ||
		!lengthM ||
		!passengers ||
		!color ||
		!timestamps ||
		!positions ||
		!bearings
	) {
		throw new Error('Trajectories Arrow schema is missing one or more required columns');
	}

	return {
		table,
		renderUnitId,
		source,
		tripId,
		routeId,
		iconKey,
		unitIndex,
		unitCount,
		isHead,
		lengthM,
		passengers,
		color,
		timestamps,
		positions,
		bearings
	};
}

export function buildActiveVehiclesAtTime(
	renderUnits: RenderUnitTable,
	t: number,
	target: ActiveVehicle[] = []
): ActiveVehicle[] {
	let count = 0;

	for (let row = 0; row < renderUnits.table.numRows; row++) {
		const timestamps = renderUnits.timestamps.get(row);
		const positions = renderUnits.positions.get(row);
		const bearings = renderUnits.bearings.get(row);
		if (!isVector(timestamps) || !isVector(positions) || !isVector(bearings)) continue;

		const bounds = findInterpolationBounds(timestamps, t);
		if (!bounds) continue;

		const position = interpolatePosition(positions, bounds.lower, bounds.upper, bounds.ratio);
		const bearing = interpolateBearing(bearings, bounds.lower, bounds.upper, bounds.ratio);
		if (!position || bearing === null) continue;

		let vehicle = target[count];
		if (!vehicle) {
			vehicle = {
				renderUnitId: '',
				source: '',
				tripId: '',
				routeId: '',
				iconKey: '',
				unitIndex: null,
				unitCount: null,
				isHead: false,
				lengthM: 0,
				passengers: null,
				color: [0, 0, 0, 255],
				position: [0, 0],
				bearing: 0
			};
			target[count] = vehicle;
		}

		vehicle.renderUnitId = String(renderUnits.renderUnitId.get(row) ?? '');
		vehicle.source = String(renderUnits.source.get(row) ?? '');
		vehicle.tripId = String(renderUnits.tripId.get(row) ?? '');
		vehicle.routeId = String(renderUnits.routeId.get(row) ?? '');
		vehicle.iconKey = String(renderUnits.iconKey.get(row) ?? '');
		vehicle.unitIndex = nullableNumber(renderUnits.unitIndex.get(row));
		vehicle.unitCount = nullableNumber(renderUnits.unitCount.get(row));
		vehicle.isHead = Boolean(renderUnits.isHead.get(row));
		vehicle.lengthM = Number(renderUnits.lengthM.get(row) ?? 0);
		vehicle.passengers = nullableNumber(renderUnits.passengers.get(row));
		const rgb = readRgb(renderUnits.color, row);
		vehicle.color[0] = rgb[0];
		vehicle.color[1] = rgb[1];
		vehicle.color[2] = rgb[2];
		vehicle.color[3] = 235;
		vehicle.position[0] = position[0];
		vehicle.position[1] = position[1];
		vehicle.bearing = bearing;
		count += 1;
	}

	target.length = count;
	return target;
}

export function normalizeBearingForIcon(bearing: number): number {
	if (!Number.isFinite(bearing)) return 0;

	return wrapDegrees(360 - bearing);
}

function nullableNumber(value: unknown): number | null {
	if (value == null) return null;
	const parsed = Number(value);
	return Number.isFinite(parsed) ? parsed : null;
}

function findInterpolationBounds(
	timestamps: Vector,
	t: number
): { lower: number; upper: number; ratio: number } | null {
	if (timestamps.length < 2) return null;

	const start = Number(timestamps.get(0));
	const end = Number(timestamps.get(timestamps.length - 1));
	if (!Number.isFinite(start) || !Number.isFinite(end)) return null;

	if (t <= start) {
		return { lower: 0, upper: 0, ratio: 0 };
	}
	if (t >= end) {
		const last = timestamps.length - 1;
		return { lower: last, upper: last, ratio: 0 };
	}

	let left = 0;
	let right = timestamps.length - 1;
	while (left <= right) {
		const mid = Math.floor((left + right) / 2);
		const value = Number(timestamps.get(mid));
		if (value === t) {
			return { lower: mid, upper: mid, ratio: 0 };
		}
		if (value < t) left = mid + 1;
		else right = mid - 1;
	}

	const upper = left;
	const lower = upper - 1;
	if (lower < 0 || upper >= timestamps.length) return null;

	const t0 = Number(timestamps.get(lower));
	const t1 = Number(timestamps.get(upper));
	if (!Number.isFinite(t0) || !Number.isFinite(t1)) return null;
	if (Math.abs(t1 - t0) < 1e-9) {
		return { lower, upper, ratio: 0 };
	}

	return {
		lower,
		upper,
		ratio: (t - t0) / (t1 - t0)
	};
}

function interpolatePosition(
	positions: Vector,
	lower: number,
	upper: number,
	ratio: number
): [number, number] | null {
	const p0 = readLonLat(positions, lower);
	if (!p0) return null;
	if (lower === upper) return p0;

	const p1 = readLonLat(positions, upper);
	if (!p1) return null;

	return [p0[0] + ratio * (p1[0] - p0[0]), p0[1] + ratio * (p1[1] - p0[1])];
}

function interpolateBearing(
	bearings: Vector,
	lower: number,
	upper: number,
	ratio: number
): number | null {
	const b0 = Number(bearings.get(lower));
	if (!Number.isFinite(b0)) return null;
	if (lower === upper) return wrapDegrees(b0);

	const b1 = Number(bearings.get(upper));
	if (!Number.isFinite(b1)) return null;

	const delta = ((b1 - b0 + 540) % 360) - 180;
	return wrapDegrees(b0 + ratio * delta);
}

function readLonLat(positions: Vector, index: number): [number, number] | null {
	const pair = positions.get(index);
	if (pair == null) return null;

	if (isVector(pair)) {
		if (pair.length < 2) return null;
		const lon = Number(pair.get(0));
		const lat = Number(pair.get(1));
		if (!Number.isFinite(lon) || !Number.isFinite(lat)) return null;
		return [lon, lat];
	}

	if (Array.isArray(pair) && pair.length >= 2) {
		const lon = Number(pair[0]);
		const lat = Number(pair[1]);
		if (!Number.isFinite(lon) || !Number.isFinite(lat)) return null;
		return [lon, lat];
	}

	return null;
}

function readRgb(colors: Vector, index: number): [number, number, number] {
	const rgb = colors.get(index);
	if (rgb == null) return [128, 128, 128];

	if (isVector(rgb)) {
		if (rgb.length < 3) return [128, 128, 128];
		return [Number(rgb.get(0) ?? 128), Number(rgb.get(1) ?? 128), Number(rgb.get(2) ?? 128)];
	}

	if (Array.isArray(rgb) && rgb.length >= 3) {
		return [Number(rgb[0] ?? 128), Number(rgb[1] ?? 128), Number(rgb[2] ?? 128)];
	}

	return [128, 128, 128];
}

function wrapDegrees(value: number): number {
	return ((value % 360) + 360) % 360;
}
