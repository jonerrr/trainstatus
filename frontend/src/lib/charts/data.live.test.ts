import type { Source, Stop, StopTime, Trip } from '#lib/client/index.js';

import { expect, test } from 'vitest';

import { buildChartData, type ChartInput } from './data.js';

// TODO: setup repo standards for frontend e2e, integration, and unit tests.
// the env var check is not a good check, i'd rather separate tests by type (e2e, integration, unit)
// Opt-in: audit every active route/direction against a running backend. Normal
// regression tests stay deterministic and never depend on live feeds.
const origin = process.env.CHART_API_ORIGIN;

async function read<T>(path: string): Promise<T> {
	const response = await fetch(new URL(path, origin), { signal: AbortSignal.timeout(20_000) });
	if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
	return response.json();
}

for (const source of ['mta_subway', 'mta_bus', 'njt_bus'] satisfies Source[]) {
	test.skipIf(!origin)(
		`${source}: all active route/direction charts preserve their itineraries`,
		async () => {
			const now = Date.now();
			const [stops, trips] = await Promise.all([
				read<Stop[]>(`/api/v1/stops/${source}`),
				read<Trip[]>(`/api/v1/trips/${source}`)
			]);
			const routes = [...new Set(trips.map((trip) => trip.route_id))];
			expect(routes.length, `${source} has no active routes to audit`).toBeGreaterThan(0);
			const stopTimes: StopTime[] = [];
			for (let offset = 0; offset < routes.length; offset += 20) {
				const query = new URLSearchParams({
					route_ids: routes.slice(offset, offset + 20).join(',')
				});
				stopTimes.push(...(await read<StopTime[]>(`/api/v1/stop_times/${source}?${query}`)));
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
				const chart = buildChartData(inputs, now);
				if (!chart.route_trips.length) continue; // No future predictions for this direction.
				checked++;
				for (const series of chart.route_trips) {
					const rows = series.points.map((point) => chart.yDomain.indexOf(point.stop_key));
					expect(
						rows.every((row, i) => row >= 0 && (i === 0 || row > rows[i - 1])),
						`${source} ${key} ${series.trip.id}: stop axis reverses`
					).toBe(true);
					const expected = (timesByTrip.get(series.trip.id) ?? [])
						.filter(
							(st) =>
								stopsById[st.stop_id] &&
								Number.isFinite(st.arrival.getTime()) &&
								st.arrival.getTime() >= now
						)
						.map((st) => `${st.stop_id}:${st.arrival.getTime()}`)
						.sort();
					expect(
						series.points.map((point) => `${point.stop_id}:${point.time.getTime()}`).sort(),
						`${source} ${key}: lost stop visits`
					).toEqual(expected);
					expect(
						series.points.every(
							(point, i) => i === 0 || point.time.getTime() >= series.points[i - 1].time.getTime()
						)
					).toBe(true);
				}
			}
			expect(checked, `${source} has no charts with future predictions`).toBeGreaterThan(0);
			console.info(
				`${source}: audited ${checked}/${graphs.size} active route/direction charts; remaining directions have no future predictions`
			);
		},
		120_000
	);
}
