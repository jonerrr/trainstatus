import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import type { CatalogRoute, CatalogStop, RouteService } from '#lib/client/index.js';
import type { IncomingTrip } from '#lib/transit/nearby.js';

/** Synthetic shared stops, duplicate services, and a farther stop with a unique route. */
export function nearbyFixture() {
	const base = chartFixture('mta_bus');
	const routes: CatalogRoute[] = ['IN', 'A', 'B', 'C', 'D', 'E'].map((id) => ({
		...base.route,
		id,
		short_name: id,
		long_name: `Route ${id}`,
		data: {
			source: 'mta_bus',
			name_prefix: '',
			name_number: 0,
			service_types: [],
			sort_key: 0,
			directions: [
				{ direction_id: 0, headsign: `${id} North` },
				{ direction_id: 1, headsign: `${id} South` }
			]
		}
	}));
	function stop(id: string, ids: string[], boardable = true): CatalogStop {
		return {
			...base.stops[0],
			id,
			name: `Board at ${id}`,
			nearby_stops: [],
			data: { source: 'mta_bus', direction: 'n', is_boardable: boardable },
			routes: ids.map((route_id) => ({
				route_id,
				stop_sequence: 0,
				data: { source: 'mta_bus', direction_ids: [0] }
			}))
		};
	}
	const origin = stop('origin', ['IN']);
	const near = stop('near', ['A']);
	const far = stop('far', ['A', 'B']);
	const third = stop('third', ['C']);
	const fourth = stop('fourth', ['D']);
	const closed = stop('closed', ['E'], false);
	const empty = stop('empty', []);
	origin.nearby_stops = [near, far, third, fourth, closed, empty].map((s, i) => ({
		source: 'mta_bus',
		stop_id: s.id,
		distance_m: (i + 1) * 20
	}));
	const catalog = {
		source: 'mta_bus' as const,
		routes,
		stops: [origin, near, far, third, fourth, closed, empty]
	};
	const incoming: IncomingTrip = {
		source: 'mta_bus',
		route_id: 'IN',
		arrival: chartNow + 60_000
	};
	function serviceFor(route_id: string, visits: [string, number, number][]): RouteService {
		return {
			trips: visits.map(([, , direction], i) => ({
				...base.trips[0],
				id: `${route_id}-${i}`,
				direction
			})),
			stops: visits.map(([stop_id]) => stop_id),
			calls: {
				trip: visits.map((_, i) => i),
				stop: visits.map((_, i) => i),
				sequence: visits.map(() => 0),
				arrival: visits.map(([, m]) => chartNow / 1000 + m * 60 - 10),
				departure: visits.map(([, m]) => chartNow / 1000 + m * 60)
			}
		};
	}
	const service = {
		source: 'mta_bus' as const,
		routes: {
			IN: serviceFor('IN', [['origin', 4, 0]]),
			A: serviceFor('A', [
				['near', 20, 0],
				['far', 5, 0]
			]),
			B: serviceFor('B', [['far', 10, 0]]),
			C: serviceFor('C', [['third', 8, 0]]),
			D: serviceFor('D', [['fourth', 3, 0]])
		}
	};
	return { catalog, service, origin, near, far, third, fourth, routes, incoming, serviceFor };
}
