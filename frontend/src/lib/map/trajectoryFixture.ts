import { tableFromArrays, tableToIPC } from 'apache-arrow';

/** Synthetic Arrow payload using the same nested columns as the wire format. */
export function trajectoryFixture(ids = ['vehicle-1']): ArrayBuffer {
	const count = ids.length;
	const values = <T>(value: T) => ids.map(() => value);
	const table = tableFromArrays({
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
		timestamps: values([100, 200]),
		positions: values([
			[-74, 40],
			[-73, 41]
		]),
		bearings: values([350, 10])
	});
	return Uint8Array.from(tableToIPC(table)).buffer;
}
