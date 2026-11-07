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
export function trajectoryFixture(ids = ['vehicle-1'], end = 200): ArrayBuffer {
	const count = ids.length;
	const values = <T>(value: T) => ids.map(() => value);
	const metadata = tableFromArrays({
		render_unit_id: ids,
		source: values('mta_subway'),
		trip_id: values('trip-1'),
		route_id: values('A'),
		icon_key: values('rail_head'),
		unit_index: new Int32Array(count),
		unit_count: values(1),
		is_head: values(true),
		length_m: values(20),
		passengers: values(12),
		color: values([0, 100, 200]),
		shape_key: values('right-angle'),
		offset_m: values(0),
		observation_vehicle: values(null),
		observed_at: values(null),
		observed_distance: values(null),
		correction_deadline: values(null)
	});
	const shared = new Table({
		shape_distances: vectorFromArray(
			ids.map((_, i) => (i === 0 ? [0, 50, 100] : [])),
			new List(new Field('item', new Float64(), false))
		),
		motion: vectorFromArray(
			ids.map((_, i) => (i === 0 ? [[100, end, 0, 0, 300, -200]] : [])),
			new List(
				new Field('item', new FixedSizeList(6, new Field('item', new Float64(), true)), true)
			)
		),
		shape_positions: vectorFromArray(
			ids.map((_, i) =>
				i === 0
					? [
							[-74, 40],
							[-73, 40],
							[-73, 41]
						]
					: []
			),
			new List(
				new Field('item', new FixedSizeList(2, new Field('item', new Float64(), true)), true)
			)
		)
	});
	return Uint8Array.from(tableToIPC(metadata.assign(shared))).buffer;
}
