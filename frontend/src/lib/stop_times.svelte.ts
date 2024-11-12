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
		try {
			const data: StopTime[] = await (
				await fetch(
					`/api/v1/stop_times${routes.length ? `?bus_route_ids=${encodeURIComponent(routes.join(','))}` : ''}`
				)
			).json();

			stop_times = data.map((stop_time) => ({
				...stop_time,
				arrival: new Date(stop_time.arrival),
				departure: new Date(stop_time.departure)
			}));

			return false;
		} catch (e) {
			console.error(e);
			return true;
		}
	}

	return {
		update,

		get stop_times() {
			return stop_times;
		}
	};
}

export const stop_times = createStopTimes();

// export const monitored_routes = $state<SvelteMap<string, string[]>>(new SvelteMap());

export const monitored_bus_routes = $state(new SvelteSet<string>());
