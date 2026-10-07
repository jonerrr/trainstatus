import type { Source, Stop, StopTime, Trip } from '#lib/client/index.js';

import { expect, test } from 'vitest';

import { buildChartData, type ChartInput } from '../../src/lib/charts/data.js';
import { readJson } from '../support/api.js';

for (const source of ['mta_subway', 'mta_bus', 'njt_bus'] satisfies Source[]) {
	test(`${source}: active route/direction charts preserve their itineraries`, async () => {
		const now = Date.now();
		const [stops, trips] = await Promise.all([
			readJson<Stop[]>(`/api/v1/stops/${source}`),
			readJson<Trip[]>(`/api/v1/trips/${source}`)
		]);
		expect(stops).toBeInstanceOf(Array);
		expect(trips).toBeInstanceOf(Array);

		const routes = [...new Set(trips.map((trip) => trip.route_id))];
		const stopTimes: StopTime[] = [];
		for (let offset = 0; offset < routes.length; offset += 20) {
			const query = new URLSearchParams({
				route_ids: routes.slice(offset, offset + 20).join(',')
			});
			stopTimes.push(...(await readJson<StopTime[]>(`/api/v1/stop_times/${source}?${query}`)));
		}
		const stopsById = Object.fromEntries(stops.map((stop) => [stop.id, stop]));
		const timesByTrip = new Map<string, StopTime[]>();
		for (const st of stopTimes) {
			const times = timesByTrip.get(st.trip_id) ?? [];
			times.push({ ...st, arrival: new Date(st.arrival), departure: new Date(st.departure) });
			timesByTrip.set(st.trip_id, times);
		}
		const graphs = new Map<string, ChartInput[]>();
		for (const trip of trips) {
			const key = `${trip.route_id}:${trip.direction}`;
			const input = graphs.get(key) ?? [];
			input.push({ trip, stopTimes: timesByTrip.get(trip.id) ?? [], stops: stopsById });
			graphs.set(key, input);
		}
		let checked = 0;
		for (const [key, inputs] of graphs) {
			const expectedVisits = new Map<string, string[]>();
			for (const { trip, stopTimes } of inputs) {
				const future = stopTimes.filter(
					(st) => Number.isFinite(st.arrival.getTime()) && st.arrival.getTime() >= now
				);
				expect(
					future.filter((st) => !stopsById[st.stop_id]),
					`${source} ${key} ${trip.id}: stop visits cannot be resolved`
				).toEqual([]);
				if (future.length) {
					expectedVisits.set(
						trip.id,
						future.map((st) => `${st.stop_id}:${st.arrival.getTime()}`).sort()
					);
				}
			}
			const chart = buildChartData(inputs, now);
			expect(
				chart.route_trips.map((series) => series.trip.id).sort(),
				`${source} ${key}: lost or unexpected trips`
			).toEqual([...expectedVisits.keys()].sort());
			if (!chart.route_trips.length) continue;
			checked++;
			for (const series of chart.route_trips) {
				const rows = series.points.map((point) => chart.yDomain.indexOf(point.stop_key));
				expect(
					rows.every((row, i) => row >= 0 && (i === 0 || row > rows[i - 1])),
					`${source} ${key} ${series.trip.id}: stop axis reverses`
				).toBe(true);
				expect(
					series.points.map((point) => `${point.stop_id}:${point.time.getTime()}`).sort(),
					`${source} ${key}: lost stop visits`
				).toEqual(expectedVisits.get(series.trip.id));
				expect(
					series.points.every(
						(point, i) => i === 0 || point.time.getTime() >= series.points[i - 1].time.getTime()
					)
				).toBe(true);
			}
		}
		console.info(
			`${source}: audited ${checked}/${graphs.size} active route/direction charts; remaining directions have no future predictions`
		);
	}, 120_000);
}
