import type {
	Catalog,
	CatalogRoute,
	CatalogStop,
	Service,
	ServiceTrip,
	Source
} from '#lib/client/index.js';

export const chartNow = new Date('2026-10-05T12:00:00Z').getTime();

// Deliberately inconsistent static sequences: short turns and branches use
// different offsets in the imported data. The timed itinerary is authoritative.
export function chartFixture(
	source: Source,
	directionIndex = 0,
	patterns = [
		['a', 'b', 'c', 'd'],
		['a', 'c', 'd'],
		['b', 'e', 'c', 'd']
	]
) {
	patterns = directionIndex === 1 ? patterns.map((pattern) => [...pattern].reverse()) : patterns;
	const route: CatalogRoute = {
		id: '4',
		short_name: '4',
		long_name: `${source} route`,
		color: source === 'mta_subway' ? '#00933c' : source === 'mta_bus' ? '#123456' : '#abcdef',
		text_color: '#ffffff',
		data:
			source === 'mta_bus'
				? {
						source,
						name_number: 4,
						name_prefix: '',
						service_types: [],
						sort_key: 0,
						directions: []
					}
				: { source }
	};
	const stops: CatalogStop[] = ['a', 'b', 'c', 'd', 'e'].map((id, i) => ({
		id,
		name: id === 'e' ? 'b' : id,
		lat: i,
		lon: i,
		nearby_stops: [],
		data:
			source === 'mta_subway'
				? {
						source,
						bubble_id: id,
						gtfs_stop_id: id,
						is_major: false,
						line: '4',
						north_headsign: '',
						south_headsign: '',
						platform_edges: [],
						station_group_id: id
					}
				: source === 'mta_bus'
					? { source, direction: 'n', is_boardable: true }
					: { source, stop_code: id },
		routes: [
			{
				route_id: route.id,
				stop_sequence: [20, 1, 30, 2, 0][i],
				data:
					source === 'mta_subway'
						? { source, stop_type: 'full_time' }
						: source === 'mta_bus'
							? { source, direction_ids: [directionIndex] }
							: { source, directions: [{ direction_id: directionIndex, headsign: '' }] }
			}
		]
	}));
	const trips: ServiceTrip[] = patterns.map((_, i) => ({
		id: `trip-${i}`,
		vehicle_id: `vehicle-${i}`,
		direction: source === 'mta_subway' ? (directionIndex === 0 ? 1 : 3) : directionIndex,
		status: null,
		data:
			source === 'mta_subway'
				? { source, consist_cars: [] }
				: source === 'njt_bus'
					? { source, headsign: '' }
					: { source }
	}));
	const stop_ids = stops.map((stop) => stop.id);
	// API column order must not define the polyline, so each trip's calls are reversed.
	const visits = patterns.flatMap((pattern, trip) =>
		pattern.map((stop_id, sequence) => ({ trip, stop_id, sequence })).reverse()
	);
	const at = (sequence: number) => chartNow / 1000 + (sequence + 1) * 60;
	const service: Service = {
		source,
		routes: {
			[route.id]: {
				trips,
				stops: stop_ids,
				calls: {
					trip: visits.map((v) => v.trip),
					stop: visits.map((v) => stop_ids.indexOf(v.stop_id)),
					sequence: visits.map((v) => v.sequence),
					arrival: visits.map((v) => at(v.sequence)),
					departure: visits.map((v) => at(v.sequence))
				}
			}
		}
	};
	const catalog: Catalog = { source, routes: [route], stops };
	return { route, stops, trips, patterns, catalog, service };
}
