import type { Route, Source, Stop, StopTime, Trip } from '#lib/client/index.js';

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
	const route: Route = {
		id: '4',
		short_name: '4',
		long_name: `${source} route`,
		color: source === 'mta_subway' ? '#00933c' : source === 'mta_bus' ? '#123456' : '#abcdef',
		text_color: '#ffffff',
		data:
			source === 'mta_bus'
				? { source, name_number: 4, name_prefix: '', service_types: [], sort_key: 0 }
				: { source }
	};
	const stops: Stop[] = ['a', 'b', 'c', 'd', 'e'].map((id, i) => ({
		id,
		name: id === 'e' ? 'b' : id,
		geom: { Point: { x: i, y: i } },
		transfers: [],
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
				stop_id: id,
				stop_sequence: [20, 1, 30, 2, 0][i],
				data:
					source === 'mta_subway'
						? { source, stop_type: 'full_time' }
						: { source, direction: directionIndex, headsign: '' }
			}
		]
	}));
	const trips: Trip[] = patterns.map((_, i) => ({
		id: `trip-${i}`,
		route_id: route.id,
		direction: source === 'mta_subway' ? (directionIndex === 0 ? 1 : 3) : directionIndex,
		created_at: new Date(chartNow),
		updated_at: new Date(chartNow),
		original_id: `trip-${i}`,
		vehicle_id: `vehicle-${i}`,
		shape_ids: [],
		data:
			source === 'mta_subway'
				? { source, consist_cars: [] }
				: source === 'njt_bus'
					? { source, headsign: '' }
					: { source }
	}));
	const stopTimes = new Map<string, StopTime[]>(
		trips.map((trip, i) => [
			trip.id,
			patterns[i]
				.map((stop_id, j) => ({
					trip_id: trip.id,
					stop_id,
					arrival: new Date(chartNow + (j + 1) * 60_000),
					departure: new Date(chartNow + (j + 1) * 60_000),
					data: { source }
				}))
				.reverse() // API array order must not define the polyline.
		])
	);
	return { route, stops, trips, stopTimes };
}
