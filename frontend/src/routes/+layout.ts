import type { Route, Stop } from '$lib/client';

import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch, data }) => {
	const { selected_sources } = data;

	const [stop_results, route_results] = await Promise.all([
		Promise.all(
			selected_sources.map(async (source) => ({
				source,
				data: (await (await fetch(`/api/v1/stops/${source}`)).json()) as Stop[]
			}))
		),
		Promise.all(
			selected_sources.map(async (source) => ({
				source,
				data: (await (await fetch(`/api/v1/routes/${source}`)).json()) as Route[]
			}))
		)
	]);

	const stops: App.PageData['stops'] = {};
	const stops_by_id: App.PageData['stops_by_id'] = {};
	for (const { source, data } of stop_results) {
		stops[source] = data;
		stops_by_id[source] = Object.fromEntries(data.map((stop) => [stop.id, stop]));
	}

	const routes: App.PageData['routes'] = {};
	const routes_by_id: App.PageData['routes_by_id'] = {};
	for (const { source, data } of route_results) {
		routes[source] = data;
		routes_by_id[source] = Object.fromEntries(data.map((route) => [route.id, route]));
	}

	return {
		stops,
		routes,
		stops_by_id,
		routes_by_id,
		...data
	};
};
