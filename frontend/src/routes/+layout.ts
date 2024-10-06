import { is_bus, is_train, type Route, type Stop } from '$lib/static';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch }) => {
	const stops_promise = fetch('/api/stops').then((res) => res.json());
	const routes_promise = fetch('/api/routes').then((res) => res.json());

	const [stops, routes]: [Stop<'bus' | 'train'>[], Route[]] = await Promise.all([
		stops_promise,
		routes_promise
	]);

	const routes_obj: {
		[id: string]: Route;
	} = {};
	for (const route of routes) {
		routes_obj[route.id] = route;
	}

	const stops_obj: {
		[id: number]: Stop<'bus' | 'train'>;
	} = {};
	for (const stop of stops) {
		stops_obj[stop.id] = stop;
	}

	const { bus_stops, train_stops } = stops.reduce(
		(acc: { bus_stops: Stop<'bus'>[]; train_stops: Stop<'train'>[] }, stop) => {
			if (is_bus(stop)) {
				acc.bus_stops.push(stop);
			} else if (is_train(stop)) {
				acc.train_stops.push(stop);
			}
			return acc;
		},
		{ bus_stops: [], train_stops: [] }
	);

	return { stops, bus_stops, train_stops, routes: routes_obj };
};
