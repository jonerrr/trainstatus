import { expect, test } from 'vitest';

import { buildActiveVehiclesAtTime, renderUnitTableFromIPC } from './trajectoryArrow';
import { trajectoryFixture } from './trajectoryFixture';

test('replays fixed frames deterministically', () => {
	const table = renderUnitTableFromIPC(trajectoryFixture());
	const frames = [100, 125, 150, 175, 200];
	const replay = () => frames.map((t) => [...buildActiveVehiclesAtTime(table, t)[0].position]);
	expect(replay()).toEqual(replay());
	console.info(
		'trajectory replay',
		JSON.stringify({ frames, positions: replay(), bytes: trajectoryFixture().byteLength })
	);
});

test.skipIf(process.env.TRAJECTORY_BENCHMARK !== '1')(
	'measures synthetic IPC, decode and frame work',
	async () => {
		const { tableFromArrays, tableFromIPC, tableToIPC } = await import('apache-arrow');
		const ids = Array.from({ length: 8 }, (_, i) => `car-${i}`);
		const times = Array.from({ length: 211 }, (_, i) => 100 + i * 2);
		const legacy = tableFromArrays({
			render_unit_id: ids,
			source: ids.map(() => 'mta_subway'),
			trip_id: ids.map(() => 'trip-1'),
			route_id: ids.map(() => 'A'),
			icon_key: ids.map(() => 'rail_car'),
			unit_index: ids.map((_, i) => i),
			unit_count: ids.map(() => 8),
			is_head: ids.map((_, i) => i === 0),
			length_m: ids.map(() => 20),
			passengers: ids.map(() => null),
			color: ids.map(() => [0, 100, 200]),
			timestamps: ids.map(() => times),
			positions: ids.map(() =>
				times.map((t) => {
					const u = (t - 100) / 420,
						s = 100 * u * u * (3 - 2 * u);
					return s <= 50 ? [-74 + s / 50, 40] : [-73, 40 + (s - 50) / 50];
				})
			),
			bearings: ids.map(() => times.map(() => 45))
		});
		const oldBytes = Uint8Array.from(tableToIPC(legacy)).buffer,
			bytes = trajectoryFixture(ids, 520);
		const loops = 200;
		let start = performance.now();
		for (let i = 0; i < loops; i++) tableFromIPC(new Uint8Array(oldBytes));
		const oldDecode = (performance.now() - start) / loops;
		start = performance.now();
		for (let i = 0; i < loops; i++) renderUnitTableFromIPC(bytes);
		const decode = (performance.now() - start) / loops;
		const legacyTable = tableFromIPC(new Uint8Array(oldBytes));
		const oldTimes = legacyTable.getChild('timestamps')!,
			oldPositions = legacyTable.getChild('positions')!,
			oldBearings = legacyTable.getChild('bearings')!;
		let checksum = 0;
		start = performance.now();
		for (let frame = 0; frame < 3000; frame++) {
			const time = 100 + (frame * 420) / 3000;
			for (let row = 0; row < legacyTable.numRows; row++) {
				const ts = oldTimes.get(row);
				let lo = 0,
					hi = ts.length - 1;
				while (lo < hi) {
					const mid = (lo + hi) >>> 1;
					if (Number(ts.get(mid)) <= time) lo = mid + 1;
					else hi = mid;
				}
				const upper = lo,
					lower = Math.max(0, upper - 1),
					u = (time - Number(ts.get(lower))) / (Number(ts.get(upper)) - Number(ts.get(lower)));
				const positions = oldPositions.get(row),
					p = positions.get(lower),
					q = positions.get(upper),
					bearings = oldBearings.get(row);
				checksum +=
					Number(p.get(0)) +
					u * (Number(q.get(0)) - Number(p.get(0))) +
					Number(p.get(1)) +
					u * (Number(q.get(1)) - Number(p.get(1))) +
					Number(bearings.get(lower));
			}
		}
		const oldMotionLookup = (performance.now() - start) / 3000;
		expect(Number.isFinite(checksum)).toBe(true);
		const table = renderUnitTableFromIPC(bytes),
			pool = buildActiveVehiclesAtTime(table, 100);
		start = performance.now();
		for (let i = 0; i < 3000; i++) buildActiveVehiclesAtTime(table, 100 + (i * 420) / 3000, pool);
		const frame = (performance.now() - start) / 3000;
		console.info(
			JSON.stringify({
				synthetic: true,
				cars: 8,
				oldSamplesPerCar: 211,
				oldIpcBytes: oldBytes.byteLength,
				newIpcBytes: bytes.byteLength,
				oldDecodeMs: oldDecode,
				newDecodeMs: decode,
				oldMotionLookupMs: oldMotionLookup,
				newFrameMs: frame,
				sharedGeometryNumbers: 9,
				motionNumbers: 6
			})
		);
		expect(bytes.byteLength).toBeLessThan(oldBytes.byteLength);
	}
);
