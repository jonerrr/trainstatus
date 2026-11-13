import type {
	CatalogRoute,
	CatalogStop,
	CatalogStopRoute,
	ServiceTrip,
	Source
} from '#lib/client/index.js';
import type { Call } from '#lib/data/decode.js';
import { calculateTextHeight } from '#lib/text-measurement.js';

/** Shared direction headsign for a trip or a static stop membership. */
export function busHeadsign(
	route: CatalogRoute | undefined,
	direction: number
): string | undefined {
	if (route?.data.source !== 'mta_bus') return undefined;

	return route.data.directions.find((d) => d.direction_id === direction)?.headsign;
}

/** Resolve destinations identically in trip cards and stop arrivals. */
export function tripHeadsign(
	trip: ServiceTrip,
	route: CatalogRoute | undefined,
	calls: readonly Call[] = [],
	stops?: ReadonlyMap<string, CatalogStop>
): string {
	if (trip.data.source === 'mta_bus') {
		return busHeadsign(route, trip.direction) || 'Unknown';
	}

	if (trip.data.source === 'njt_bus') {
		const headsign = trip.data.headsign;
		if (headsign) return headsign;

		// Static route-stop labels may describe the opposite direction or a
		// different branch. Only use labels on this trip in its own direction.
		for (const call of calls) {
			const routeStop = stops
				?.get(call.stopId)
				?.routes.find(
					(rs) =>
						rs.route_id === call.routeId &&
						rs.data.source === 'njt_bus' &&
						rs.data.directions.some((d) => d.direction_id === trip.direction && d.headsign)
				);
			if (routeStop?.data.source === 'njt_bus') {
				const headsign = routeStop.data.directions.find(
					(d) => d.direction_id === trip.direction
				)?.headsign;
				if (headsign) return headsign;
			}
		}
	}

	const lastStop = calls[calls.length - 1];
	return (lastStop && stops?.get(lastStop.stopId)?.name) || 'Unknown';
}

/** Title shared by trip modals and map cards when a headsign is missing. */
export function tripDestinationTitle(
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
	let lastCall = -Infinity;
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
		lastCall = Date.now();
		func(...args);
	};
	const throttled = (...args: Parameters<T>) => {
		pending = args;
		const remaining = lastCall + wait - Date.now();
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
export const mainRouteStops = (
	routeStops: readonly CatalogStopRoute[],
	activeRoutes?: ReadonlySet<string>
): CatalogStopRoute[] => {
	return routeStops.filter(
		(route) =>
			route.data.source !== 'mta_subway' ||
			(['full_time', 'part_time'].includes(route.data.stop_type) &&
				(!route.route_id.endsWith('X') || !activeRoutes || activeRoutes.has(route.route_id)))
	);
};

const ROUTE_ROW_HEIGHT = 54;
// Trip/Button reserves 68px including the occupancy meter; List adds p-2 and two 1px borders.
const TRIP_ROW_HEIGHT = 68 + 16 + 2;
// p-2 on the stop row (8px top + 8px bottom)
const STOP_ROW_PADDING = 16;
// One bus route: icon row plus its first direction label and ETA row
const BUS_ROUTE_HEIGHT = 56;
const BUS_EXTRA_LABEL_HEIGHT = 24;
// One arrival row (route icon + ETA chips)
const SUBWAY_ARRIVAL_ROW_HEIGHT = 24;

export const calculateRouteHeight = () => ROUTE_ROW_HEIGHT;

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

export function calculateStopHeight(item: CatalogStop) {
	// 16px total padding (p-2 = 8px top + 8px bottom)
	let height = STOP_ROW_PADDING;

	// Stop name — may wrap on narrow screens
	height += calculateTextHeight(
		item.name,
		STOP_NAME_FONT,
		STOP_NAME_MAX_WIDTH,
		STOP_NAME_LINE_HEIGHT
	);

	if (item.data.source === 'mta_bus' || item.data.source === 'njt_bus') {
		// Reserve every static label even when the next trip shows only one direction.
		height += item.routes.reduce((total, route) => {
			const labels =
				route.data.source === 'mta_bus'
					? route.data.direction_ids.length
					: route.data.source === 'njt_bus'
						? route.data.directions.length
						: 1;
			return total + BUS_ROUTE_HEIGHT + (Math.max(1, labels) - 1) * BUS_EXTRA_LABEL_HEIGHT;
		}, 0);
	} else if (item.data.source === 'mta_subway') {
		// Two direction columns — measure the taller headsign
		const northH = calculateTextHeight(
			item.data.north_headsign,
			HEADSIGN_FONT,
			HEADSIGN_MAX_WIDTH,
			HEADSIGN_LINE_HEIGHT
		);
		const southH = calculateTextHeight(
			item.data.south_headsign,
			HEADSIGN_FONT,
			HEADSIGN_MAX_WIDTH,
			HEADSIGN_LINE_HEIGHT
		);
		height += Math.max(northH, southH);

		// Route arrival rows (24px each)
		const mainRoutes = mainRouteStops(item.routes);
		height += mainRoutes.length * SUBWAY_ARRIVAL_ROW_HEIGHT;
	}

	return height;
}

export const calculateTripHeight = () => TRIP_ROW_HEIGHT;

/**
 * Height of a list row skeleton: the same constants as the calculators above, for the
 * smallest real row (one headsign line, one route), so skeletons can't drift from them.
 */
export function skeletonRowHeight(type: 'stop' | 'route' | 'trip', source?: Source) {
	if (type === 'route') return calculateRouteHeight();
	if (type === 'trip') return calculateTripHeight();
	const base = STOP_ROW_PADDING + STOP_NAME_LINE_HEIGHT;
	return source === 'mta_subway'
		? base + HEADSIGN_LINE_HEIGHT + SUBWAY_ARRIVAL_ROW_HEIGHT
		: base + BUS_ROUTE_HEIGHT;
}
