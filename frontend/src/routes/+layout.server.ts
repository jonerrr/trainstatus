import type { Route, Source, Stop } from '$lib/client';
import { COOKIE_NAME, parse_sources, supported_sources } from '$lib/source_preferences.svelte';

import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ fetch, url, cookies }) => {
	const cookie_value = cookies.get(COOKIE_NAME);
	let selected_sources = parse_sources(cookie_value);

	// Deep-link auto-enable: if URL has 'src' param, include it temporarily
	const src_param = url.searchParams.get('src');
	if (
		src_param &&
		supported_sources.includes(src_param as Source) &&
		!selected_sources.includes(src_param as Source)
	) {
		selected_sources = [...selected_sources, src_param as Source];
	}

	const at = url.searchParams.get('at') ?? undefined;

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
		selected_sources,
		stops,
		routes,
		stops_by_id,
		routes_by_id,
		at
	};
};
