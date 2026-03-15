import { index_alerts } from '$lib/resources/alerts.svelte';
import { index_positions } from '$lib/resources/positions.svelte';
import { index_stop_times } from '$lib/resources/stop_times.svelte';
import { index_trips } from '$lib/resources/trips.svelte';
import { COOKIE_NAME, parse_sources, supported_sources } from '$lib/source_preferences.svelte';

import type { Route, Source, Stop } from '@trainstatus/client';

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
	const rt_params = at ? `?at=${at}` : '';

	// Fetch stops and routes for selected sources in parallel
	const [
		stop_results,
		route_results,
		initial_trips,
		initial_stop_times,
		initial_positions,
		initial_alerts
	] = await Promise.all([
		///// static sources
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
		),
		///// realtime sources
		Promise.all(
			selected_sources.map(async (source) => ({
				source,
				data: index_trips(await (await fetch(`/api/v1/trips/${source}${rt_params}`)).json())
			}))
		),
		Promise.all(
			selected_sources.map(async (source) => ({
				source,
				data: index_stop_times(
					await (await fetch(`/api/v1/stop_times/${source}${rt_params}`)).json()
				)
			}))
		),
		Promise.all(
			selected_sources.map(async (source) => ({
				source,
				data: index_positions(await (await fetch(`/api/v1/positions/${source}${rt_params}`)).json())
			}))
		),
		Promise.all(
			selected_sources.map(async (source) => ({
				source,
				data: index_alerts(await (await fetch(`/api/v1/alerts/${source}${rt_params}`)).json())
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
		at,
		initial_trips,
		initial_stop_times,
		initial_positions,
		initial_alerts
	};
};
