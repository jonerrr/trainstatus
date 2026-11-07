import type { MotionInterval, Source } from '#lib/client/index.js';

import {
	Field,
	FixedSizeList,
	Float64,
	List,
	Table,
	tableFromArrays,
	tableToIPC,
	vectorFromArray
} from 'apache-arrow';

/** Synthetic Arrow payload using the same nested columns as the wire format. */
export function trajectoryFixture(
	ids = ['vehicle-1'],
	end = 200,
	{
		source = 'mta_subway',
		positions = [
			[-74, 40],
			[-73, 40],
			[-73, 41]
		],
		distances = [0, 50, 100],
		motion = [{ t0: 100, t1: end, a: 0, b: 0, c: 300, d: -200 }],
		color = [0, 100, 200],
		lengthM = 20
	}: {
		source?: Source;
		positions?: [number, number][];
		distances?: number[];
		motion?: MotionInterval[];
		color?: [number, number, number];
		lengthM?: number;
	} = {}
): ArrayBuffer {
	const count = ids.length;
	const values = <T>(value: T) => ids.map(() => value);
	const metadata = tableFromArrays({
		render_unit_id: ids,
		source: values(source),
		trip_id: values('trip-1'),
		route_id: values('A'),
		icon_key: ids.map((_, i) =>
			source === 'mta_subway' ? (i === 0 ? 'rail_head' : 'rail_car') : 'bus'
		),
		unit_index: Int32Array.from(ids, (_, i) => i),
		unit_count: values(count),
		is_head: ids.map((_, i) => i === 0),
		length_m: values(lengthM),
		passengers: values(12),
		color: values(color),
		shape_key: values('right-angle'),
		offset_m: values(0)
	});
	const shared = new Table({
		shape_distances: vectorFromArray(
			ids.map((_, i) => (i === 0 ? distances : [])),
			new List(new Field('item', new Float64(), false))
		),
		motion: vectorFromArray(
			ids.map((_, i) => (i === 0 ? motion.map((p) => [p.t0, p.t1, p.a, p.b, p.c, p.d]) : [])),
			new List(
				new Field('item', new FixedSizeList(6, new Field('item', new Float64(), true)), true)
			)
		),
		shape_positions: vectorFromArray(
			ids.map((_, i) => (i === 0 ? positions : [])),
			new List(
				new Field('item', new FixedSizeList(2, new Field('item', new Float64(), true)), true)
			)
		)
	});
	return Uint8Array.from(tableToIPC(metadata.assign(shared))).buffer;
}
