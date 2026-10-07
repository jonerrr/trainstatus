import type { TypedStopTime } from '#lib/resources/index.svelte.js';

import { describe, expect, it } from 'vitest';

import { index_stop_times } from './stop_times.svelte';

function stopTime(trip_id: string, stop_id: string, arrival: Date): TypedStopTime<'mta_subway'> {
	return {
		trip_id,
		stop_id,
		arrival,
		departure: arrival,
		data: { source: 'mta_subway' }
	};
}

describe('index_stop_times', () => {
	it('indexes the same visits by trip and by stop, with converted dates', () => {
		const first = new Date('2026-09-07T00:00:00Z');
		const second = new Date('2026-09-07T00:02:00Z');
		const result = index_stop_times([
			stopTime('trip-1', 'stop-a', first),
			stopTime('trip-1', 'stop-b', second)
		]);

		expect(result.by_trip_id.get('trip-1')?.map((st) => st.stop_id)).toEqual(['stop-a', 'stop-b']);
		expect(result.by_stop_id.get('stop-a')?.map((st) => st.trip_id)).toEqual(['trip-1']);
		expect(result.by_stop_id.get('stop-b')?.map((st) => st.trip_id)).toEqual(['trip-1']);
		expect(result.by_trip_id.get('trip-1')?.[0].arrival).toBeInstanceOf(Date);
		expect(result.by_trip_id.get('trip-1')?.[0].arrival).not.toBe(first);
		expect(result.by_trip_id.get('trip-1')?.[0].arrival.getTime()).toBe(first.getTime());
		expect(result.by_trip_id.get('trip-1')?.[1].departure.getTime()).toBe(second.getTime());
	});
});
