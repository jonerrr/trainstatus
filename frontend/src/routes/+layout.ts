import { update_data } from '$lib/api';
import { stops, trips, stop_times, alerts, bus_stops, bus_routes } from '$lib/stores';
import type { LayoutLoad } from './$types';

// load data here first for SSR benefits (i think)
export const load: LayoutLoad = async ({ fetch, url }) => {
	const stopsPromise = fetch('/api/stops').then((res) => res.json());
	const busStopsPromise = fetch('/api/bus/stops').then((res) => res.json());
	const busRoutesPromise = fetch('/api/bus/routes').then((res) => res.json());

	const initDataPromise = update_data(fetch, trips, stop_times, alerts, null);

	const [stops_res, bus_stops_res, bus_routes_res] = await Promise.all([
		stopsPromise,
		busStopsPromise,
		busRoutesPromise,
		initDataPromise
	]);

	stops.set(stops_res);
	bus_stops.set(bus_stops_res);
	bus_routes.set(bus_routes_res);
};
