import type {
	Catalog,
	CatalogRoute,
	CatalogStop,
	RouteService,
	Service,
	ServiceTrip,
	Source
} from '#lib/client/index.js';

/** One stop visit of a trip. Times are epoch milliseconds. */
export interface Call {
	trip_id: string;
	route_id: string;
	stop_id: string;
	sequence: number;
	arrival: number;
	departure: number;
}

/** Live data older than this since its fetch or successful source commit is stale. */
export const STALE_AFTER_MS = 90_000;

/** A route's complete service, indexed once when installed. */
export interface RouteData {
	source: Source;
	route_id: string;
	/** Requested historical time in Unix seconds; null for live data. */
	at: number | null;
	/** Client time of the successful fetch, epoch milliseconds; null when never fetched. */
	fetched_at: number | null;
	/** Successful source commit, epoch milliseconds; absent on synthetic or historical data. */
	source_updated_at?: number | null;
	trips: ReadonlyMap<string, ServiceTrip>;
	/** Calls of each trip in visit order. */
	calls_by_trip: ReadonlyMap<string, readonly Call[]>;
	/** Calls at each stop in arrival order. */
	calls_by_stop: ReadonlyMap<string, readonly Call[]>;
}

export interface CatalogData {
	source: Source;
	routes: readonly CatalogRoute[];
	stops: readonly CatalogStop[];
	routes_by_id: ReadonlyMap<string, CatalogRoute>;
	stops_by_id: ReadonlyMap<string, CatalogStop>;
}

export function emptyRoute(
	source: Source,
	route_id: string,
	at: number | null = null,
	fetched_at: number | null = null
): RouteData {
	return {
		source,
		route_id,
		at,
		fetched_at,
		trips: new Map(),
		calls_by_trip: new Map(),
		calls_by_stop: new Map()
	};
}

/** Future predictions remain visible regardless of age. */
export function upcomingCalls(calls: readonly Call[], now: number): Call[] {
	return calls.filter((call) => Math.max(call.arrival, call.departure) >= now);
}

/** Whether live route data has gone unrefreshed past the stale threshold. */
export function isStale(route: RouteData, now: number): boolean {
	if (route.at !== null || route.fetched_at === null) return false;
	const refreshed_at = Math.min(route.fetched_at, route.source_updated_at ?? route.fetched_at);
	return now - refreshed_at > STALE_AFTER_MS;
}

function index(name: string, value: number, length: number) {
	if (!Number.isInteger(value) || value < 0 || value >= length) {
		throw new Error(`Service ${name} index ${value} is out of bounds`);
	}
}

export function decodeRoute(
	source: Source,
	route_id: string,
	route: RouteService,
	at: number | null = null,
	fetched_at: number | null = null
): RouteData {
	const { trips, stops, calls } = route;
	const length = calls.trip.length;
	for (const column of [calls.stop, calls.sequence, calls.arrival, calls.departure]) {
		if (column.length !== length)
			throw new Error(`Service columns for ${route_id} differ in length`);
	}
	const calls_by_trip = new Map<string, Call[]>(trips.map((trip) => [trip.id, []]));
	const calls_by_stop = new Map<string, Call[]>();
	for (let i = 0; i < length; i++) {
		index('trip', calls.trip[i], trips.length);
		index('stop', calls.stop[i], stops.length);
		const call: Call = {
			trip_id: trips[calls.trip[i]].id,
			route_id,
			stop_id: stops[calls.stop[i]],
			sequence: calls.sequence[i],
			arrival: calls.arrival[i] * 1000,
			departure: calls.departure[i] * 1000
		};
		calls_by_trip.get(call.trip_id)!.push(call);
		let at_stop = calls_by_stop.get(call.stop_id);
		if (!at_stop) calls_by_stop.set(call.stop_id, (at_stop = []));
		at_stop.push(call);
	}
	for (const at_stop of calls_by_stop.values()) at_stop.sort((a, b) => a.arrival - b.arrival);
	return {
		source,
		route_id,
		at,
		fetched_at,
		trips: new Map(trips.map((trip) => [trip.id, trip])),
		calls_by_trip,
		calls_by_stop
	};
}

export function decodeService(body: Service, fetched_at: number): Map<string, RouteData> {
	const at = body.at ?? null;
	return new Map(
		Object.entries(body.routes).map(([id, route]) => [
			id,
			{
				...decodeRoute(body.source, id, route, at, fetched_at),
				source_updated_at: at === null && body.updated_at != null ? body.updated_at * 1000 : null
			}
		])
	);
}

export function decodeCatalog(body: Catalog): CatalogData {
	return {
		source: body.source,
		routes: body.routes,
		stops: body.stops,
		routes_by_id: new Map(body.routes.map((route) => [route.id, route])),
		stops_by_id: new Map(body.stops.map((stop) => [stop.id, stop]))
	};
}

/** Merge several routes' calls at a stop into one arrival-ordered list. */
export function callsAtStop(routes: Iterable<RouteData>, stop_id: string): Call[] {
	const merged: Call[] = [];
	for (const route of routes) merged.push(...(route.calls_by_stop.get(stop_id) ?? []));
	return merged.sort((a, b) => a.arrival - b.arrival);
}
