import type { ServiceTrip } from '#lib/client/index.js';

import type { Call, RouteData } from './decode.js';

export type StopArrival = Call & { trip: ServiceTrip; eta: number };

/** Resolve arrivals and route membership together so badges and rows agree. */
export function stopArrivals(
	calls: readonly Call[],
	routes: ReadonlyMap<string, RouteData> | readonly RouteData[],
	now: number,
	options: { showPrevious?: boolean; includeDueNow?: boolean } = {}
): { arrivals: StopArrival[]; activeRoutes: ReadonlySet<string> } {
	const { showPrevious = false, includeDueNow = true } = options;
	const byId =
		routes instanceof Map
			? (routes as ReadonlyMap<string, RouteData>)
			: new Map((routes as readonly RouteData[]).map((route) => [route.routeId, route]));
	const arrivals: StopArrival[] = [];
	const activeRoutes = new Set<string>();
	for (const call of calls) {
		if (!showPrevious && (call.arrival < now || (!includeDueNow && call.arrival === now))) continue;
		const trip = byId.get(call.routeId)?.trips.get(call.tripId);
		if (!trip) continue;
		arrivals.push({ ...call, trip, eta: (call.arrival - now) / 60_000 });
		activeRoutes.add(call.routeId);
	}
	return { arrivals, activeRoutes };
}
