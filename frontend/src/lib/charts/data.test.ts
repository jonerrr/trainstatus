import type { Source } from '#lib/client/index.js';
import { decodeCatalog, decodeRoute } from '#lib/transit/decode.js';

import { describe, expect, it } from 'vitest';

import { buildChartData, type ChartInput } from './data.js';
import { chartFixture, chartNow } from './fixtures.js';

function inputs(source: Source, patterns?: string[][]): ChartInput[] {
	const fixture = chartFixture(source, 0, patterns);
	const route = decodeRoute(source, '4', fixture.service.routes['4']);
	const { stops_by_id } = decodeCatalog(fixture.catalog);
	return fixture.trips.map((trip) => ({
		source,
		route_id: '4',
		trip,
		calls: [...route.calls_by_trip.get(trip.id)!].reverse(),
		stops: stops_by_id
	}));
}

function checkItineraries(input: ChartInput[]) {
	const chart = buildChartData(input, chartNow);
	expect(chart.route_trips).toHaveLength(input.length);
	for (let i = 0; i < input.length; i++) {
		const points = chart.route_trips[i].points;
		// Preserve the actual itinerary, rather than sorting points into axis order
		// or silently discarding visits to satisfy the monotonicity assertion.
		const timedStops = [...input[i].calls].sort((a, b) => a.arrival - b.arrival);
		expect(points.map((p) => p.stop_id)).toEqual(timedStops.map((st) => st.stop_id));
		expect(points.map((p) => p.time)).toEqual(timedStops.map((st) => new Date(st.arrival)));
		const rows = points.map((p) => chart.yDomain.indexOf(p.stop_key));
		expect(rows.every((row, j) => row >= 0 && (j === 0 || row > rows[j - 1]))).toBe(true);
	}
	return chart;
}

describe('chart stop ordering', () => {
	for (const source of ['mta_subway', 'mta_bus', 'njt_bus'] satisfies Source[]) {
		it(`${source}: aligns local, express and branch stops despite conflicting static sequences`, () => {
			const chart = checkItineraries(inputs(source));
			expect(chart.yDomain.map((key) => chart.stopNames.get(key))).toEqual([
				'a',
				'b',
				'b',
				'c',
				'd'
			]);
			expect(new Set(chart.yDomain).size).toBe(5);
		});
		it(`${source}: keeps every visit in a loop and conflicting service patterns`, () => {
			const chart = checkItineraries(
				inputs(source, [
					['a', 'b', 'a', 'c'],
					['a', 'c', 'b']
				])
			);
			expect(chart.yDomain.map((key) => chart.stopNames.get(key))).toEqual([
				'a',
				'b',
				'a (2)',
				'c',
				'b (2)'
			]);
		});
	}

	it('does not merge stop IDs or trip IDs from different sources', () => {
		const chart = checkItineraries([
			...inputs('mta_subway', [['a', 'b']]),
			...inputs('njt_bus', [['a', 'b']])
		]);
		expect(chart.yDomain).toHaveLength(4);
	});

	it('keeps the axis stable when API trip and stop-time arrays are shuffled', () => {
		const input = inputs('mta_bus');
		const original = buildChartData(input, chartNow);
		const shuffled = buildChartData(
			[...input].reverse().map((item) => ({ ...item, calls: [...item.calls].reverse() })),
			chartNow
		);
		expect(shuffled.yDomain).toEqual(original.yDomain);
	});

	it('shares each stop once when a later branch resolves an initially ambiguous order', () => {
		const chart = checkItineraries(
			inputs('mta_bus', [
				['a', 'b', 'd', 'e'],
				['a', 'c', 'd', 'e'],
				['a', 'c', 'b']
			])
		);
		expect(chart.yDomain.map((key) => chart.stopNames.get(key))).toEqual(['a', 'c', 'b', 'd', 'b']);
		expect(chart.yDomain).toHaveLength(5);
	});

	it('filters past visits, unknown stops and invalid dates without inventing points', () => {
		const input = inputs('njt_bus', [['a', 'b', 'c']]);
		input[0].calls = [
			...input[0].calls,
			{ ...input[0].calls[0], stop_id: 'missing' },
			{ ...input[0].calls[0], arrival: NaN }
		];
		const chart = buildChartData(input, chartNow + 120_000);
		expect(chart.route_trips[0].points.map((p) => p.stop_id)).toEqual(['b', 'c']);
		expect(chart.yDomain.map((key) => chart.stopNames.get(key))).toEqual(['b', 'c']);
		expect(buildChartData(input, chartNow + 600_000).route_trips).toEqual([]);
		expect(buildChartData([], chartNow).yDomain).toEqual([]);
	});

	it('accepts simultaneous arrivals without losing either stop', () => {
		const input = inputs('mta_bus', [['a', 'b']]);
		input[0].calls = input[0].calls.map((st) => ({
			...st,
			arrival: chartNow,
			departure: chartNow
		}));
		const chart = buildChartData(input, chartNow);
		expect(chart.route_trips[0].points.map((p) => p.stop_id)).toEqual(['a', 'b']);
		expect(chart.yDomain).toHaveLength(2);
	});

	it('resolves simultaneous predictions using the travel order established by other trips', () => {
		const input = inputs('mta_bus', [
			['b', 'a', 'c'],
			['b', 'a', 'c']
		]);
		input[1].calls = input[1].calls.map((st) =>
			st.stop_id === 'a' ? { ...st, arrival: chartNow + 60_000, departure: chartNow + 60_000 } : st
		);
		const chart = buildChartData(input, chartNow);
		expect(chart.yDomain.map((key) => chart.stopNames.get(key))).toEqual(['b', 'a', 'c']);
		expect(chart.route_trips.map((series) => series.points.map((point) => point.stop_id))).toEqual([
			['b', 'a', 'c'],
			['b', 'a', 'c']
		]);
	});

	it('preserves all 120 permutations of five stops, including incompatible patterns', () => {
		function permutations(ids: string[]): string[][] {
			return ids.length
				? ids.flatMap((id, i) =>
						permutations(ids.filter((_, j) => j !== i)).map((rest) => [id, ...rest])
					)
				: [[]];
		}
		const patterns = permutations(['a', 'b', 'c', 'd', 'e']);
		expect(patterns).toHaveLength(120);
		checkItineraries(inputs('mta_subway', patterns));
	});

	it('does not let an unrelated loop reverse the resolved order of equal predictions', () => {
		const input = inputs('mta_bus', [
			['b', 'c', 'a'],
			['b', 'c', 'a'],
			['d', 'e', 'd']
		]);
		input[1].calls = input[1].calls.map((st) =>
			st.stop_id === 'a'
				? { ...st, arrival: chartNow + 120_000, departure: chartNow + 120_000 }
				: st
		);
		const chart = buildChartData(input, chartNow);
		expect(
			chart.route_trips.slice(0, 2).map((series) => series.points.map((point) => point.stop_id))
		).toEqual([
			['b', 'c', 'a'],
			['b', 'c', 'a']
		]);
	});
});
