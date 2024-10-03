export interface StopTime<T = never, D = never, R = never> {
	trip_id: string;
	stop_id: number;
	arrival: Date;
	departure: Date;
	eta: T;
	direction: D;
	route_id: R;
}

export function createStopTimes() {
	let stop_times: StopTime[] = $state([]);

	function update(routes: string[]) {
		fetch(
			`/api/stop_times${routes.length ? `?bus_route_ids=${encodeURIComponent(routes.join(','))}` : ''}`
		)
			.then((res) => res.json())
			.then(
				(data) =>
					// convert dates from strings to Date objects
					(stop_times = data.map((stop_time: StopTime) => ({
						...stop_time,
						arrival: new Date(stop_time.arrival),
						departure: new Date(stop_time.departure)
					})))
			);
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
export const monitored_routes = $state<string[]>([]);

export function stop_arrivals(times: StopTime[], stop_id: number) {
	return times.filter((time) => time.stop_id === stop_id);
}
