import { type Route, type Stop, is_bus, is_train } from '$lib/static';
import { trips } from '$lib/trips.svelte';
import { stop_times } from '$lib/stop_times.svelte';
import type { LayoutLoad } from './$types';
import { alerts } from '$lib/alerts.svelte';

export const load: LayoutLoad = async ({ fetch }) => {
	const stops_promise = fetch('/api//v1/stops').then((res) => res.json());
	const routes_promise = fetch('/api/v1/routes').then((res) => res.json());

	// TODO: preload bus route ids using search param

	const [stops, routes]: [Stop<'bus' | 'train'>[], Route[], void, void, boolean] =
		await Promise.all([
			stops_promise,
			routes_promise,
			trips.update(fetch),
			stop_times.update(fetch, []),
			alerts.update(fetch)
		]);

	// let longest_bus_route_name = routes.reduce((acc, route) => {
	// 	if (route.short_name.length > acc.length) {
	// 		return route.short_name;
	// 	}
	// 	return acc;
	// }, '');
	// console.log(longest_bus_route_name);
	// const longest_stop_name = stops.reduce((acc, stop) => {
	// 	if (stop.name.length > acc.length) {
	// 		return stop.name;
	// 	}
	// 	return acc;
	// }, '');
	// console.log(longest_stop_name);
	// const sorted_stop_names = stops.map((stop) => stop.name.length).sort((a, b) => b - a);
	// console.log(sorted_stop_names);

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

	return {
		stops: stops_obj,
		bus_stops,
		train_stops,
		routes: routes_obj
		// initial_promise: Promise.all([trips.update(fetch), stop_times.update(fetch, [])])
	};
};
