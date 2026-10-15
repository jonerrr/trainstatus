import type { Catalog, Service, Source } from '#lib/client/index.js';
import { decodeCatalog, decodeService } from '#lib/transit/decode.js';

import { expect, test } from 'vitest';

import { buildChartData, type ChartInput } from '../../src/lib/charts/data.js';
import { readJson } from '../support/api.js';

for (const source of ['mta_subway', 'mta_bus', 'njt_bus'] satisfies Source[]) {
	test(`${source}: active route/direction charts preserve their itineraries`, async () => {
		const now = Date.now();
		const catalog = await readJson<Catalog>(`/api/v1/catalog/${source}`);
		if (!catalog.routes.length) {
			console.info(`${source}: no catalog routes to audit`);
			return;
		}
		const query =
			source === 'mta_subway'
				? ''
				: `?${new URLSearchParams({ route_ids: catalog.routes.map((route) => route.id).join(',') })}`;
		const service = await readJson<Service>(`/api/v1/service/${source}${query}`);
		const stopsById = decodeCatalog(catalog).stops_by_id;
		const graphs = new Map<string, ChartInput[]>();
		for (const [route_id, route] of decodeService(service, now)) {
			for (const trip of route.trips.values()) {
				const key = `${route_id}:${trip.direction}`;
				const input = graphs.get(key) ?? [];
				const calls = route.calls_by_trip.get(trip.id) ?? [];
				input.push({ source, route_id, trip, calls, stops: stopsById });
				graphs.set(key, input);
			}
		}
		let checked = 0;
		for (const [key, inputs] of graphs) {
			const expectedVisits = new Map<string, string[]>();
			for (const { trip, calls } of inputs) {
				const future = calls.filter((st) => Number.isFinite(st.arrival) && st.arrival >= now);
				expect(
					future.filter((st) => !stopsById.has(st.stop_id)),
					`${source} ${key} ${trip.id}: stop visits cannot be resolved`
				).toEqual([]);
				if (future.length) {
					expectedVisits.set(trip.id, future.map((st) => `${st.stop_id}:${st.arrival}`).sort());
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
