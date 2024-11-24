import { SvelteSet } from 'svelte/reactivity';

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

	async function update(fetch: Fetch, routes: string[]) {
		// try {
		const res = await fetch(
			`/api/v1/stop_times${routes.length ? `?bus_route_ids=${encodeURIComponent(routes.join(','))}` : ''}`
		);
		if (res.headers.has('x-sw-fallback')) {
			throw new Error('Offline');
		}
		const data: StopTime[] = await res.json();

		stop_times = data.map((stop_time) => ({
			...stop_time,
			arrival: new Date(stop_time.arrival),
			departure: new Date(stop_time.departure)
		}));

		// return false;
		// } catch (e) {
		// 	console.error(e);
		// 	return true;
		// }
	}

	return {
		update,

		get stop_times() {
			return stop_times;
		}
	};
}

export const stop_times = createStopTimes();

export const monitored_bus_routes = $state(new SvelteSet<string>());
