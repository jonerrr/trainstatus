import { SvelteMap } from 'svelte/reactivity';

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
					`/api/stop_times${routes.length ? `?bus_route_ids=${encodeURIComponent(routes.join(','))}` : ''}`
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

		// fetch(
		// 	`/api/stop_times${routes.length ? `?bus_route_ids=${encodeURIComponent(routes.join(','))}` : ''}`
		// )
		// 	.then((res) => res.json())
		// 	.then(e
		// 		(data) =>
		// 			// convert dates from strings to Date objects
		// 			(stop_times = data.map((stop_time: StopTime) => ({
		// 				...stop_time,
		// 				arrival: new Date(stop_time.arrival),
		// 				departure: new Date(stop_time.departure)
		// 			})))
		// 	);
		// TODO: add error handling and set offline status
	}

	return {
		update,

		get stop_times() {
			return stop_times;
		}
	};
}

export const stop_times = createStopTimes();
// export const monitored_routes = $state<string[]>([]);

export const monitored_routes = $state<SvelteMap<string, string[]>>(new SvelteMap());
