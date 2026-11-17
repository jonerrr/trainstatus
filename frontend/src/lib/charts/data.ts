import type { CatalogStop, ServiceTrip, Source } from '#lib/client/index.js';
import type { Call } from '#lib/data/decode.js';

import type { ChartPoint, ChartSeries } from './context.js';

export type ChartInput = {
	source: Source;
	routeId: string;
	trip: ServiceTrip;
	calls: readonly Call[];
	stops: ReadonlyMap<string, CatalogStop>;
};

/** Merge itineraries without changing either one's travel order. Repeated
 * stops remain separate visits; conflicting patterns get additional axis rows.
 * An ordinary local/express/branch combination shares its common stops. */
function mergeStops(axis: string[], pattern: string[]): string[] {
	const lengths = Array.from(
		{ length: axis.length + 1 },
		() => new Uint32Array(pattern.length + 1)
	);
	for (let i = axis.length - 1; i >= 0; i--) {
		for (let j = pattern.length - 1; j >= 0; j--) {
			lengths[i][j] =
				axis[i] === pattern[j]
					? 1 + lengths[i + 1][j + 1]
					: Math.max(lengths[i + 1][j], lengths[i][j + 1]);
		}
	}
	const merged: string[] = [];
	let i = 0;
	let j = 0;
	while (i < axis.length && j < pattern.length) {
		if (axis[i] === pattern[j]) {
			merged.push(axis[i++]);
			j++;
		} else if (lengths[i + 1][j] >= lengths[i][j + 1]) {
			merged.push(axis[i++]);
		} else {
			merged.push(pattern[j++]);
		}
	}
	return merged.concat(axis.slice(i), pattern.slice(j));
}

/** Build deduplicated travel-order edges and incoming counts for each stop. */
function stopGraph(patterns: string[][]) {
	const edges = new Map<string, Set<string>>();
	const indegrees = new Map<string, number>();
	for (const pattern of patterns) {
		for (const stop of pattern) {
			if (!edges.has(stop)) {
				edges.set(stop, new Set());
				indegrees.set(stop, 0);
			}
		}
		for (let i = 1; i < pattern.length; i++) {
			const next = pattern[i];
			const outgoing = edges.get(pattern[i - 1])!;
			if (!outgoing.has(next)) {
				outgoing.add(next);
				indegrees.set(next, indegrees.get(next)! + 1);
			}
		}
	}
	return { edges, indegrees };
}

/** Each timed itinerary contributes precedence constraints. A topological
 * order shares all stops exactly once whenever the patterns are compatible. */
function orderStops(patterns: string[][]): string[] {
	const { edges, indegrees } = stopGraph(patterns);
	const ready = [...indegrees.keys()].filter((stop) => indegrees.get(stop) === 0).sort();
	const axis: string[] = [];
	while (ready.length) {
		const stop = ready.shift()!;
		axis.push(stop);
		for (const next of edges.get(stop)!) {
			const degree = indegrees.get(next)! - 1;
			indegrees.set(next, degree);
			if (degree === 0) ready.push(next);
		}
		ready.sort();
	}
	if (axis.length === edges.size) return axis;
	// A loop or conflicting itinerary cannot use one row per stop. Merge full
	// patterns into a common supersequence so no actual visit is lost or reordered.
	return patterns.reduce((axis, pattern) => mergeStops(axis, pattern), [] as string[]);
}

/** `[start, end]` in ms: `now ± span`, with the start pulled forward to the earliest recorded visit. */
export function chartXDomain(
	trips: readonly Pick<ChartSeries, 'points'>[],
	now: number,
	span: number
): [number, number] {
	let earliest = now;
	for (const { points } of trips) {
		for (const point of points) earliest = Math.min(earliest, point.time.getTime());
	}
	return [Math.max(earliest, now - span), now + span];
}

/** Preserve timed visits and align trip itineraries on a shared stop axis. */
export function buildChartData(inputs: readonly ChartInput[]) {
	const names = new Map<string, string>();
	const series = inputs.map(({ source, routeId, trip, calls, stops }) => ({
		source,
		routeId,
		trip,
		points: [...calls]
			.filter((call) => stops.has(call.stopId) && Number.isFinite(call.arrival))
			.sort((a, b) => a.sequence - b.sequence)
			.map((call) => {
				const stop = stops.get(call.stopId)!;
				const stopKey = JSON.stringify([source, stop.id]);
				names.set(stopKey, stop.name);
				return {
					stopId: stop.id,
					stopKey,
					stopName: stop.name,
					time: new Date(call.arrival)
				};
			})
	}));

	// Input iteration order must not change the axis. Prefer the most complete
	// itinerary, then merge each distinct short turn, express or branch pattern.
	const patterns = [...new Set(series.map((s) => JSON.stringify(s.points.map((p) => p.stopKey))))]
		.map((encoded) => ({ encoded, stops: JSON.parse(encoded) as string[] }))
		.sort((a, b) => b.stops.length - a.stops.length || a.encoded.localeCompare(b.encoded));
	const axis = orderStops(patterns.map((pattern) => pattern.stops));
	const occurrences = new Map<string, number>();
	const stopNames = new Map<string, string>();
	const axisKeys = axis.map((stop) => {
		const visit = (occurrences.get(stop) ?? 0) + 1;
		occurrences.set(stop, visit);
		const key = JSON.stringify([stop, visit]);
		stopNames.set(key, names.get(stop)! + (visit > 1 ? ` (${visit})` : ''));
		return key;
	});

	const visible = new Set<string>();
	const routeTrips = series
		.map(({ source, routeId, trip, points }): ChartSeries => {
			let index = 0;
			const visits: ChartPoint[] = [];
			for (const point of points) {
				while (axis[index] !== point.stopKey) index++;
				const stopKey = axisKeys[index++];
				visible.add(stopKey);
				visits.push({
					stopId: point.stopId,
					stopKey,
					stopName: point.stopName,
					time: point.time
				});
			}
			return { source, routeId, trip, points: visits };
		})
		.filter((s) => s.points.length);
	return { routeTrips, yDomain: axisKeys.filter((key) => visible.has(key)), stopNames };
}
