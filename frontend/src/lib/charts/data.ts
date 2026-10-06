import type { Stop, StopTime, Trip } from '#lib/client/index.js';

import type { ChartPoint } from './context.js';

export type ChartInput = {
	trip: Trip;
	stopTimes: readonly StopTime[];
	stops: Record<string, Stop>;
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

/** Collapse strongly connected stops before ranking equal predictions. Order
 * within a loop is ambiguous, but it must not disturb definite order elsewhere. */
function rankDefiniteStops(patterns: string[][]): Map<string, number> {
	const { edges } = stopGraph(patterns);
	const indices = new Map<string, number>();
	const lows = new Map<string, number>();
	const stack: string[] = [];
	const pending = new Set<string>();
	const groups = new Map<string, string[]>();
	const groupFor = new Map<string, string>();

	function visit(stop: string) {
		const index = indices.size;
		indices.set(stop, index);
		lows.set(stop, index);
		stack.push(stop);
		pending.add(stop);
		for (const next of edges.get(stop)!) {
			if (!indices.has(next)) {
				visit(next);
				lows.set(stop, Math.min(lows.get(stop)!, lows.get(next)!));
			} else if (pending.has(next)) {
				lows.set(stop, Math.min(lows.get(stop)!, indices.get(next)!));
			}
		}
		if (lows.get(stop) !== index) return;
		const members: string[] = [];
		let member: string;
		do {
			member = stack.pop()!;
			pending.delete(member);
			members.push(member);
		} while (member !== stop);
		members.sort();
		const id = members[0];
		groups.set(id, members);
		for (const member of members) groupFor.set(member, id);
	}
	for (const stop of [...edges.keys()].sort()) {
		if (!indices.has(stop)) visit(stop);
	}
	const condensed = [...groups.keys()].map((group) => [group]);
	for (const [stop, nextStops] of edges) {
		for (const next of nextStops) {
			const from = groupFor.get(stop)!;
			const to = groupFor.get(next)!;
			if (from !== to) condensed.push([from, to]);
		}
	}
	return new Map(
		orderStops(condensed)
			.flatMap((group) => groups.get(group)!)
			.map((stop, index) => [stop, index])
	);
}

export function buildChartData(inputs: readonly ChartInput[], now: number) {
	const names = new Map<string, string>();
	const series = inputs.map(({ trip, stopTimes, stops }) => ({
		trip,
		points: [...stopTimes]
			.filter((st) => stops[st.stop_id] && Number.isFinite(st.arrival.getTime()))
			.sort(
				(a, b) =>
					a.arrival.getTime() - b.arrival.getTime() ||
					a.departure.getTime() - b.departure.getTime() ||
					a.stop_id.localeCompare(b.stop_id)
			)
			.map((st) => {
				const stop = stops[st.stop_id];
				const stop_key = JSON.stringify([trip.data.source, stop.id]);
				names.set(stop_key, stop.name);
				return {
					stop_id: stop.id,
					stop_key,
					stop_name: stop.name,
					time: st.arrival,
					departure: st.departure.getTime()
				};
			})
	}));

	const samePrediction = (
		a: (typeof series)[number]['points'][number],
		b: (typeof series)[number]['points'][number]
	) => a.time.getTime() === b.time.getTime() && a.departure === b.departure;
	if (
		series.some((s) => s.points.some((point, i) => i > 0 && samePrediction(point, s.points[i - 1])))
	) {
		// Equal predictions supply no order evidence. Only connect different time
		// groups, then let definite observations from other trips resolve the ties.
		const definite: string[][] = [];
		for (const { points } of series) {
			let previous: string[] = [];
			for (let i = 0; i < points.length;) {
				let end = i + 1;
				while (end < points.length && samePrediction(points[i], points[end])) end++;
				const group = points.slice(i, end).map((point) => point.stop_key);
				for (const stop of group) {
					definite.push([stop]);
					for (const before of previous) definite.push([before, stop]);
				}
				previous = group;
				i = end;
			}
		}
		definite.sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b)));
		const rank = rankDefiniteStops(definite);
		for (const { points } of series) {
			points.sort(
				(a, b) =>
					a.time.getTime() - b.time.getTime() ||
					a.departure - b.departure ||
					rank.get(a.stop_key)! - rank.get(b.stop_key)!
			);
		}
	}

	// Input iteration order must not change the axis. Prefer the most complete
	// itinerary, then merge each distinct short turn, express or branch pattern.
	const patterns = [...new Set(series.map((s) => JSON.stringify(s.points.map((p) => p.stop_key))))]
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
	const route_trips = series
		.map(({ trip, points }) => {
			let index = 0;
			const visits: ChartPoint[] = [];
			for (const point of points) {
				while (axis[index] !== point.stop_key) index++;
				const stop_key = axisKeys[index++];
				if (point.time.getTime() < now) continue;
				visible.add(stop_key);
				visits.push({
					stop_id: point.stop_id,
					stop_key,
					stop_name: point.stop_name,
					time: point.time
				});
			}
			return { trip, points: visits };
		})
		.filter((s) => s.points.length);
	return { route_trips, yDomain: axisKeys.filter((key) => visible.has(key)), stopNames };
}
