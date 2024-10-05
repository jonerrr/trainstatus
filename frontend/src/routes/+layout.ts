import type { Route, Stop } from '$lib/static';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch }) => {
	const stops_promise = fetch('/api/stops').then((res) => res.json());
	const routes_promise = fetch('/api/routes').then((res) => res.json());

	const [stops, routes]: [Stop<'bus' | 'train'>[], Route[]] = await Promise.all([
		stops_promise,
		routes_promise
	]);

	// put routes into a map
	const route_map = new Map<string, Route>();
	for (const route of routes) {
		route_map.set(route.id, route);
	}

	return { stops, routes: route_map };
};
