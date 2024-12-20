import { SvelteSet } from 'svelte/reactivity';
import { current_time } from './util.svelte';

export interface StopTime<T = never, D = never, R = never> {
	trip_id: string;
	stop_id: number;
	arrival: Date;
	departure: Date;
	eta: T;
	direction: D;
	route_id: R;
}

type Fetch = typeof fetch;

export function createStopTimes() {
	let stop_times: StopTime[] = $state([]);

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
		const data: StopTime[] = (await res.json()).map((stop_time: StopTime) => ({
			...stop_time,
			arrival: new Date(stop_time.arrival),
			departure: new Date(stop_time.departure)
		}));

		const remove_stop_ids = new Set(data.map((st) => st.trip_id));

		// if only_bus, we need to preserve stop_times for train
		if (only_bus) {
			const not_updated = stop_times.filter((st) => !remove_stop_ids.has(st.trip_id));
			stop_times = data.concat(not_updated);
		} else {
			stop_times = data;
			// stop_times = data.map((stop_time) => ({
			// 	...stop_time,
			// 	arrival: new Date(stop_time.arrival),
			// 	departure: new Date(stop_time.departure)
			// }));
		}
	}

	// async function add_bus_routes(fetch: Fetch, routes: Set<string>) {
	// 	const new_routes = routes.difference(monitored_bus_routes);

	// 	if (new_routes.size === 0) return;
	// 	console.log('adding ', new_routes);
	// }

	return {
		update,
		// add_bus_routes,

		get stop_times() {
			return stop_times;
		}
	};
}

export const stop_times = createStopTimes();

export const monitored_bus_routes = $state(new SvelteSet<string>());
