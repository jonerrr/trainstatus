import type { ServiceTrip } from '#lib/client/index.js';

import type { Call, RouteData } from './decode.js';

export type StopArrival = Call & { trip: ServiceTrip; eta: number };

/** Resolve arrivals and route membership together so badges and rows agree. */
export function stop_arrivals(
	calls: readonly Call[],
	routes: ReadonlyMap<string, RouteData> | readonly RouteData[],
	now: number,
	options: { show_previous?: boolean; include_due_now?: boolean } = {}
): { arrivals: StopArrival[]; active_routes: ReadonlySet<string> } {
	const { show_previous = false, include_due_now = true } = options;
	const by_id =
		routes instanceof Map
			? (routes as ReadonlyMap<string, RouteData>)
			: new Map((routes as readonly RouteData[]).map((route) => [route.route_id, route]));
	const arrivals: StopArrival[] = [];
	const active_routes = new Set<string>();
	for (const call of calls) {
		if (!show_previous && (call.arrival < now || (!include_due_now && call.arrival === now)))
			continue;
		const trip = by_id.get(call.route_id)?.trips.get(call.trip_id);
		if (!trip) continue;
		arrivals.push({ ...call, trip, eta: (call.arrival - now) / 60_000 });
		active_routes.add(call.route_id);
	}
	return { arrivals, active_routes };
}
