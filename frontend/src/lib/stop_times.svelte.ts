import { SvelteSet } from 'svelte/reactivity';

import { page } from '$app/state';

import type { Trip } from '$lib/trips.svelte';

import { resource } from 'runed';

// goal
// const stopTimes = useStopTimes
// stopTimes.at = Date.now()
// const arrivals = stopTimes.getArrivals(stop_id)
// const departures = stopTimes.getDepartures(stop_id)
// can have like route.existing_alert that is reactive

// Global state for monitored routes (kept as requested)
export const monitored_bus_routes = new SvelteSet<string>();

// Helper to index stop times (same as before)
function indexStopTimes(data: StopTime[]) {
	const byTrip: Record<string, StopTime[]> = {};
	const byStop: Record<number, StopTime[]> = {};

	for (const st of data) {
		if (typeof st.arrival === 'string') st.arrival = new Date(st.arrival);
		if (typeof st.departure === 'string') st.departure = new Date(st.departure);
		(byTrip[st.trip_id] ??= []).push(st);
		(byStop[st.stop_id] ??= []).push(st);
	}
	return { byTrip, byStop };
}

interface StopTimeOptions {
	requireRoutes?: boolean; // If true, only fetch if routes are monitored (e.g. buses)
	monitoredRoutes?: SvelteSet<string>;
}

// // export function createStopTimes(
// // 	source: string,
// // 	params: { at: number },
// // 	options: StopTimeOptions = {}
// // ) {
// // 	const { requireRoutes = false, monitoredRoutes } = options;

// 	const stResource = resource(
// 		() => ({
// 			at: params.at,
// 			// If we don't care about routes (Subway), pass null so it doesn't trigger updates
// 			routes: requireRoutes && monitoredRoutes ? Array.from(monitoredRoutes) : null
// 		}),
// 		async ({ at, routes }, prev, { signal, data: prevData }) => {
// 			// 1. If we require routes but have none, return empty
// 			if (requireRoutes && (!routes || routes.length === 0)) {
// 				return [];
// 			}

// 			const query = new URLSearchParams();
// 			if (at) query.set('at', at.toString());

// 			// 2. Logic for partial updates (Buses)
// 			let fetchRoutes = routes;
// 			let isPartialUpdate = false;

// 			// If time hasn't changed, but routes HAVE changed, we might only need new ones
// 			if (requireRoutes && prev && prev.at === at && prevData && prev.routes) {
// 				const prevSet = new Set(prev.routes);
// 				const newRoutes = routes!.filter(r => !prevSet.has(r));

// 				if (newRoutes.length > 0) {
// 					fetchRoutes = newRoutes;
// 					isPartialUpdate = true;
// 				} else if (routes!.length < prev.routes.length) {
// 					// Routes were removed: We can just filter the existing data without fetching
// 					const currentSet = new Set(routes);
// 					// This logic depends on your API, but usually filtering client side is fine here
// 					// For simplicity in this example, we'll just return the filtered previous data
// 					// Note: Realistically you might want to re-fetch to be safe, but this saves bandwidth
// 					return prevData;
// 				}
// 			}

// 			// 3. Prepare Query
// 			if (requireRoutes && fetchRoutes) {
// 				query.set('route_ids', fetchRoutes.join(','));
// 			}

// 			const res = await fetch(`/api/v1/stop_times/${source}?${query}`, { signal });
// 			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');

// 			const newData: StopTime[] = await res.json();

// 			// 4. Merge Logic
// 			if (isPartialUpdate && prevData) {
// 				const newTripIds = new Set(newData.map(st => st.trip_id));
// 				return [
// 					...prevData.filter(st => !newTripIds.has(st.trip_id)),
// 					...newData
// 				];
// 			}

// 			return newData;
// 		},
// 		{
// 			initialValue: [],
// 			debounce: 100 // Slight debounce for UI sliders
// 		}
// 	);

// 	// Derived indexes that update automatically
// 	const indexes = $derived(indexStopTimes(stResource.current ?? []));

// // 	return {
// // 		get stop_times() { return stResource.current; },
// // 		get by_trip_id() { return indexes.byTrip; },
// // 		get by_stop_id() { return indexes.byStop; },
// // 		get loading() { return stResource.loading; },
// // 		get error() { return stResource.error; },
// // 		update: stResource.refetch
// // 	};
// // }

export interface StopTime<T = never | Trip> {
	trip_id: string;
	stop_id: number;
	arrival: Date;
	departure: Date;
	trip: T;
	// eta: T;
	// direction: D;
	// route_id: R;
}

type Fetch = typeof fetch;

// type ById<K extends string | number> = {
// 	[P in K]: number[];
// };
type ByStopId = {
	[stop_id: number]: StopTime[];
};

type ByTripId = {
	[trip_id: string]: StopTime[];
};

export function createStopTimes() {
	let stop_times: StopTime[] = $state([]);

	// let filter_arrivals = $state(false);
	// <trip_id, index in array above>
	let st_by_trip_id: ByTripId = $state({});
	let st_by_stop_id: ByStopId = $state({});

	// there are way too many bus routes to load all at once, so we only fetch the routes that the user is looking at.
	// const monitored_bus_routes = $state(new SvelteSet<string>());
	// used to show skeleton loader while updating
	let updating_bus_routes = $state(new SvelteSet<string>());

	// must specify routes if only_bus is true
	async function update(
		fetch: Fetch,
		routes: string[],
		only_bus: boolean = false,
		at?: string
		// finished: boolean = false
	) {
		if (only_bus) {
			updating_bus_routes = new SvelteSet(routes);
		}
		// TODO: if only_bus was fetched too recently, don't include buses in next request
		const params = new URLSearchParams();
		if (routes.length) {
			params.set('bus_route_ids', routes.join(','));
			if (only_bus) {
				params.set('only_bus', 'true');
			}
		}
		if (at) {
			// convert back to seconds from ms
			params.set('at', at.toString());
		}

		// if on charts, fetch finished stop times
		// if (finished) {
		// 	params.set('finished', 'true');
		// }

		const res = await fetch(`/api/v1/stop_times${params.size ? '?' + params.toString() : ''}`);
		if (res.headers.has('x-sw-fallback')) {
			throw new Error('Offline');
		}
		const data: StopTime[] = await res.json();

		const st_by_trip_id_new: ByTripId = {};
		const st_by_stop_id_new: ByStopId = {};
		// TODO: maybe move this to below
		for (let i = 0; i < data.length; i++) {
			const stop_time = data[i];
			stop_time.arrival = new Date(stop_time.arrival);
			stop_time.departure = new Date(stop_time.departure);

			if (!st_by_trip_id_new[stop_time.trip_id]) {
				st_by_trip_id_new[stop_time.trip_id] = [];
			}
			if (!st_by_stop_id_new[stop_time.stop_id]) {
				st_by_stop_id_new[stop_time.stop_id] = [];
			}

			st_by_trip_id_new[stop_time.trip_id].push(stop_time);
			st_by_stop_id_new[stop_time.stop_id].push(stop_time);
		}
		// console.log(st_by_trip_id);
		if (only_bus) {
			const result: StopTime[] = [];
			const trip_ids = new Set<string>();
			const stop_ids = new Set<number>();
			for (const st of data) {
				trip_ids.add(st.trip_id);
				stop_ids.add(st.stop_id);
			}

			// Keep existing non-bus stop times
			for (const st of stop_times) {
				if (!trip_ids.has(st.trip_id)) {
					result.push(st);
					st_by_trip_id_new[st.trip_id] = st_by_trip_id[st.trip_id];
				}
				if (!stop_ids.has(st.stop_id)) {
					st_by_stop_id_new[st.stop_id] = st_by_stop_id[st.stop_id];
				}
			}

			// Add new bus stop times
			for (const st of data) {
				result.push(st);
			}

			stop_times = result;
		} else {
			stop_times = data;
		}
		st_by_trip_id = st_by_trip_id_new;
		st_by_stop_id = st_by_stop_id_new;
		updating_bus_routes = new SvelteSet();
	}

	// function add_routes(routes: string[]) {
	// 	// add routes and remove duplicates too
	// 	monitored_bus_routes.push(...new Set(routes));
	// 	// monitored_bus_routes.add(route);
	// }

	// async function add_bus_routes(fetch: Fetch, routes: Set<string>) {
	// 	const new_routes = routes.difference(monitored_bus_routes);

	// 	if (new_routes.size === 0) return;
	// 	console.log('adding ', new_routes);
	// }

	return {
		update,
		// add_bus_routes,
		get by_trip_id() {
			return st_by_trip_id;
		},

		get by_stop_id() {
			return st_by_stop_id;
		},

		// set monitored_routes(routes: string[]) {
		// 	monitored_bus_routes.clear();
		// 	monitored_bus_routes.add(...routes);
		// },

		// get monitored_routes() {
		// 	return monitored_bus_routes;
		// },

		get updating_routes() {
			return updating_bus_routes;
		},

		// set filter_arrivals(value: boolean) {
		// 	// can only be set once, if user spams button we don't want to keep updating
		// 	// TODO: maybe debounce this instead
		// 	if (filter_arrivals) return;
		// 	filter_arrivals = value;
		// },

		// get filter_arrivals() {
		// 	return filter_arrivals;
		// },

		get stop_times() {
			return stop_times;
		}
	};
}

export const stop_times = createStopTimes();

// export const monitored_bus_routes = $state(new SvelteSet<string>());
