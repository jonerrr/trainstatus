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
	tripId: string;
	routeId: string;
	stopId: string;
	sequence: number;
	arrival: number;
	departure: number;
	platformEdges: readonly string[];
}

/** Live data older than this since its fetch or successful source commit is stale. */
export const STALE_AFTER_MS = 90_000;

/** A route's complete service, indexed once when installed. */
export interface RouteData {
	source: Source;
	routeId: string;
	/** Requested historical time in Unix seconds; undefined for live data. */
	at: number | undefined;
	/** Client time of the successful fetch, epoch milliseconds; null when never fetched. */
	fetchedAt: number | null;
	/** Successful source commit, epoch milliseconds; absent on synthetic or historical data. */
	sourceUpdatedAt?: number | null;
	trips: ReadonlyMap<string, ServiceTrip>;
	/** Calls of each trip in visit order. */
	callsByTrip: ReadonlyMap<string, readonly Call[]>;
	/** Calls at each stop in arrival order. */
	callsByStop: ReadonlyMap<string, readonly Call[]>;
}

export interface CatalogData {
	source: Source;
	routes: readonly CatalogRoute[];
	stops: readonly CatalogStop[];
	routesById: ReadonlyMap<string, CatalogRoute>;
	stopsById: ReadonlyMap<string, CatalogStop>;
}

/** Future predictions remain visible regardless of age. */
export function upcomingCalls(calls: readonly Call[], now: number): Call[] {
	return calls.filter((call) => Math.max(call.arrival, call.departure) >= now);
}

function index(name: string, value: number, length: number) {
	if (!Number.isInteger(value) || value < 0 || value >= length) {
		throw new Error(`Service ${name} index ${value} is out of bounds`);
	}
}

export function decodeRoute(
	source: Source,
	routeId: string,
	route: RouteService,
	at: number | undefined = undefined,
	fetchedAt: number | null = null
): RouteData {
	const { trips, stops, calls } = route;
	const length = calls.trip.length;
	for (const column of [
		calls.stop,
		calls.sequence,
		calls.arrival,
		calls.departure,
		...(calls.platform_edges ? [calls.platform_edges] : [])
	]) {
		if (column.length !== length)
			throw new Error(`Service columns for ${routeId} differ in length`);
	}
	const callsByTrip = new Map<string, Call[]>(trips.map((trip) => [trip.id, []]));
	const callsByStop = new Map<string, Call[]>();
	for (let i = 0; i < length; i++) {
		index('trip', calls.trip[i], trips.length);
		index('stop', calls.stop[i], stops.length);
		const call: Call = {
			tripId: trips[calls.trip[i]].id,
			routeId,
			stopId: stops[calls.stop[i]],
			sequence: calls.sequence[i],
			arrival: calls.arrival[i] * 1000,
			departure: calls.departure[i] * 1000,
			platformEdges: calls.platform_edges?.[i] ?? []
		};
		callsByTrip.get(call.tripId)!.push(call);
		let atStop = callsByStop.get(call.stopId);
		if (!atStop) callsByStop.set(call.stopId, (atStop = []));
		atStop.push(call);
	}
	for (const atStop of callsByStop.values()) atStop.sort((a, b) => a.arrival - b.arrival);
	return {
		source,
		routeId,
		at,
		fetchedAt,
		trips: new Map(trips.map((trip) => [trip.id, trip])),
		callsByTrip,
		callsByStop
	};
}

export function decodeService(body: Service, fetchedAt: number): Map<string, RouteData> {
	const at = body.at ?? undefined;
	return new Map(
		Object.entries(body.routes).map(([id, route]) => [
			id,
			{
				...decodeRoute(body.source, id, route, at, fetchedAt),
				sourceUpdatedAt: at === undefined && body.updated_at != null ? body.updated_at * 1000 : null
			}
		])
	);
}

export function decodeCatalog(body: Catalog): CatalogData {
	return {
		source: body.source,
		routes: body.routes,
		stops: body.stops,
		routesById: new Map(body.routes.map((route) => [route.id, route])),
		stopsById: new Map(body.stops.map((stop) => [stop.id, stop]))
	};
}

/** Merge several routes' calls at a stop into one arrival-ordered list. */
export function callsAtStop(routes: Iterable<RouteData>, stopId: string): Call[] {
	const merged: Call[] = [];
	for (const route of routes) merged.push(...(route.callsByStop.get(stopId) ?? []));
	return merged.sort((a, b) => a.arrival - b.arrival);
}
