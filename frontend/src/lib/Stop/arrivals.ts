import type { StopTime, Trip } from '#lib/client/index.js';

export type StopArrival = StopTime & { trip: Trip; eta: number };

/** Resolve arrivals and route membership together so badges and rows agree. */
export function get_stop_arrivals(
	stop_times: readonly StopTime[],
	trips: ReadonlyMap<string, Trip> | undefined,
	now: number,
	options: { show_previous?: boolean; include_due_now?: boolean } = {}
): { arrivals: StopArrival[]; active_routes: ReadonlySet<string> } {
	const { show_previous = false, include_due_now = true } = options;
	const arrivals: StopArrival[] = [];
	const active_routes = new Set<string>();

	for (const st of stop_times) {
		const arrival = st.arrival.getTime();
		if (!show_previous && (arrival < now || (!include_due_now && arrival === now))) continue;
		const trip = trips?.get(st.trip_id);
		if (!trip) continue;

		arrivals.push({ ...st, trip, eta: (arrival - now) / 60_000 });
		active_routes.add(trip.route_id);
	}

	return { arrivals, active_routes };
}
