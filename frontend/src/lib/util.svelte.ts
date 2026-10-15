import type {
	CatalogRoute,
	CatalogStop,
	CatalogStopRoute,
	ServiceTrip
} from '#lib/client/index.js';
import { calculateTextHeight } from '#lib/text_measurement.js';
import type { Call } from '#lib/transit/decode.js';

/**
 * Destination shown on an MTA bus for a given trip direction.
 *
 * The upstream feed maps headsigns to directions at the route level, not to
 * individual stops, so route-stop headsigns are always empty for buses.
 * Resolve from the route's directions using the trip's `direction` instead.
 */
export function bus_headsign(
	route: CatalogRoute | undefined,
	direction: number
): string | undefined {
	if (route?.data.source !== 'mta_bus') return undefined;

	const match = route.data.directions?.find((d) => d.direction_id === direction);
	if (!match) return undefined;

	return match.via?.length ? `${match.destination} via ${match.via.join('/')}` : match.destination;
}

/** Resolve destinations identically in trip cards and stop arrivals. */
export function trip_headsign(
	trip: ServiceTrip,
	route: CatalogRoute | undefined,
	calls: readonly Call[] = [],
	stops?: ReadonlyMap<string, CatalogStop>
): string {
	if (trip.data.source === 'mta_bus') {
		// TODO: why make this a separate function when it's only used here?
		return bus_headsign(route, trip.direction) || 'Unknown';
	}

	if (trip.data.source === 'njt_bus') {
		const headsign = trip.data.headsign;
		if (headsign) return headsign;

		// Static route-stop labels may describe the opposite direction or a
		// different branch. Only use labels on this trip in its own direction.
		for (const call of calls) {
			const route_stop = stops
				?.get(call.stop_id)
				?.routes.find(
					(rs) =>
						rs.route_id === call.route_id &&
						rs.data.source === 'njt_bus' &&
						rs.data.direction === trip.direction &&
						rs.data.headsign
				);
			if (route_stop?.data.source === 'njt_bus') return route_stop.data.headsign;
		}
	}

	const last_stop = calls[calls.length - 1];
	return (last_stop && stops?.get(last_stop.stop_id)?.name) || 'Unknown';
}

/** Title shared by trip modals and map cards when a headsign is missing. */
export function trip_destination_title(
	destination: string | undefined,
	route?: CatalogRoute
): string {
	return destination && destination !== 'Unknown'
		? destination
		: route?.long_name || 'Destination unavailable';
}

// from https://www.geeksforgeeks.org/haversine-formula-to-find-distance-between-two-points-on-a-sphere/
export function haversine(lat1: number, lon1: number, lat2: number, lon2: number) {
	// distance between latitudes and longitudes
	const dLat = ((lat2 - lat1) * Math.PI) / 180.0;
	const dLon = ((lon2 - lon1) * Math.PI) / 180.0;

	// convert to radians
	lat1 = (lat1 * Math.PI) / 180.0;
	lat2 = (lat2 * Math.PI) / 180.0;

	// apply formula
	const a =
		Math.pow(Math.sin(dLat / 2), 2) +
		Math.pow(Math.sin(dLon / 2), 2) * Math.cos(lat1) * Math.cos(lat2);
	const rad = 6371;
	const c = 2 * Math.asin(Math.sqrt(a));
	return rad * c;
}

/** Run immediately, then at most once per interval using the latest arguments. */
export function throttle<T extends (...args: never[]) => void>(func: T, wait: number) {
	let last_call = -Infinity;
	let timeout: ReturnType<typeof setTimeout> | undefined;
	let pending: Parameters<T> | undefined;
	const cancel = () => {
		clearTimeout(timeout);
		timeout = undefined;
		pending = undefined;
	};
	const invoke = () => {
		const args = pending!;
		cancel();
		last_call = Date.now();
		func(...args);
	};
	const throttled = (...args: Parameters<T>) => {
		pending = args;
		const remaining = last_call + wait - Date.now();
		if (remaining <= 0) invoke();
		else if (timeout === undefined) timeout = setTimeout(invoke, remaining);
	};
	return Object.assign(throttled, { cancel });
}

/** Create during component initialization; get/set bind the input, current feeds consumers. */
export function createThrottled<T>(initial: T, wait: number) {
	let input = $state.raw(initial);
	let current = $state.raw(initial);
	const update = throttle((value: T) => {
		current = value;
	}, wait);
	$effect(() => update.cancel);

	return {
		get: () => input,
		set: (value: T) => {
			input = value;
			update(value);
		},
		get current() {
			return current;
		}
	};
}

export function debounce<T extends (...args: never[]) => void>(func: T, wait: number = 75) {
	let timeout: ReturnType<typeof setTimeout> | undefined;
	const cancel = () => {
		clearTimeout(timeout);
		timeout = undefined;
	};
	const debounced = (...args: Parameters<T>) => {
		cancel();
		timeout = setTimeout(() => {
			timeout = undefined;
			func(...args);
		}, wait);
	};
	return Object.assign(debounced, { cancel });
}

// Get main routes for a stop, optionally hiding express variants without service.
export const main_route_stops = (
	route_stops: readonly CatalogStopRoute[],
	active_routes?: ReadonlySet<string>
): CatalogStopRoute[] => {
	return route_stops.filter(
		(route) =>
			route.data.source !== 'mta_subway' ||
			(['full_time', 'part_time'].includes(route.data.stop_type) &&
				(!route.route_id.endsWith('X') || !active_routes || active_routes.has(route.route_id)))
	);
};

export const calculate_route_height = () => 54;

// Fonts matching the Tailwind classes used in Stop/Button.svelte
// text-lg font-medium → 500 18px/28px Inter
const STOP_NAME_FONT = '500 18px Inter, sans-serif';
const STOP_NAME_LINE_HEIGHT = 28;
// font-semibold → 600 16px/24px Inter (headsign labels)
const HEADSIGN_FONT = '600 16px Inter, sans-serif';
const HEADSIGN_LINE_HEIGHT = 24;

// Approximate container width for stop name text.
// The list item has p-2 (8px each side) and the pin button takes ~36px on the right.
// On most mobile viewports (375px) this gives ~375 - 16 - 36 ≈ 323px.
// Using a conservative default; the actual width rarely matters because stop
// names are short, but this catches the occasional long bus-stop name.
const STOP_NAME_MAX_WIDTH = 300;
// Headsigns inside the 2-column grid get roughly half the container width
const HEADSIGN_MAX_WIDTH = 140;

export function calculate_stop_height(item: CatalogStop) {
	// 16px total padding (p-2 = 8px top + 8px bottom)
	let height = 16;

	// Stop name — may wrap on narrow screens
	height += calculateTextHeight(
		item.name,
		STOP_NAME_FONT,
		STOP_NAME_MAX_WIDTH,
		STOP_NAME_LINE_HEIGHT
	);

	if (item.data.source === 'mta_bus' || item.data.source === 'njt_bus') {
		// Each bus route row: icon (20px) + headsign text + ETAs line ≈ 56px fixed
		height += item.routes.length * 56;
	} else if (item.data.source === 'mta_subway') {
		// Two direction columns — measure the taller headsign
		const north_h = calculateTextHeight(
			item.data.north_headsign,
			HEADSIGN_FONT,
			HEADSIGN_MAX_WIDTH,
			HEADSIGN_LINE_HEIGHT
		);
		const south_h = calculateTextHeight(
			item.data.south_headsign,
			HEADSIGN_FONT,
			HEADSIGN_MAX_WIDTH,
			HEADSIGN_LINE_HEIGHT
		);
		height += Math.max(north_h, south_h);

		// Route arrival rows (24px each)
		const main_routes = main_route_stops(item.routes);
		height += main_routes.length * 24;
	}

	return height;
}

export const calculate_trip_height = () => 94;
