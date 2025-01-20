import { SvelteSet } from 'svelte/reactivity';
import { current_time } from './util.svelte';
import type { Trip } from './trips.svelte';

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
	// let by_trip_id = $state(new SvelteMap<string, number[]>());
	// let by_stop_id = $state(new SvelteMap<number, number[]>());

	// must specify routes if only_bus is true
	async function update(fetch: Fetch, routes: string[], only_bus: boolean = false) {
		// TODO: if only_bus was fetched too recently, don't include buses in next request
		const params = new URLSearchParams();
		if (routes.length) {
			params.set('bus_route_ids', routes.join(','));
			if (only_bus) {
				params.set('only_bus', 'true');
			}
		}
		if (current_time.value) {
			// convert back to seconds from ms
			params.set('at', current_time.value.toString());
		}

		const res = await fetch(`/api/v1/stop_times${params.size ? '?' + params.toString() : ''}`);
		if (res.headers.has('x-sw-fallback')) {
			throw new Error('Offline');
		}
		const data: StopTime[] = await res.json();
		// const data: StopTime[] = (await res.json()).map((stop_time: StopTime) => ({
		// 	...stop_time,
		// 	arrival: new Date(stop_time.arrival),
		// 	departure: new Date(stop_time.departure)
		// }));

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
	}

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

export const monitored_bus_routes = $state(new SvelteSet<string>());
