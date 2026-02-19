import { searchSchema } from '$lib/params.schema';
import { index_alerts } from '$lib/sources/alerts.svelte';
import { default_sources } from '$lib/sources/index.svelte';
import { index_positions } from '$lib/sources/positions.svelte';
import { index_stop_times } from '$lib/sources/stop_times.svelte';
import { index_trips } from '$lib/sources/trips.svelte';

import type { Route, Stop } from '@trainstatus/client';
import { validateSearchParams } from 'runed/kit';

import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch, url }) => {
	const { searchParams } = validateSearchParams(url, searchSchema);
	// print all search params
	const params_obj = Object.fromEntries(searchParams.entries());
	console.dir({ params_obj });

	const at = searchParams.get('at') ?? undefined;

	// Fetch stops and routes for all sources in parallel
	const [stop_results, route_results, initial_trips, initial_stop_times, initial_positions, initial_alerts] =
		await Promise.all([
			Promise.all(
				default_sources.map(async (source) => ({
					source,
					data: (await (await fetch(`/api/v1/stops/${source}`)).json()) as Stop[]
				}))
			),
			Promise.all(
				default_sources.map(async (source) => ({
					source,
					data: (await (await fetch(`/api/v1/routes/${source}`)).json()) as Route[]
				}))
			),
			// TODO: check if monitored route needs to be added from query param
			// TODO: include ?at in rt fetches
			Promise.all(
				default_sources.map(async (source) => ({
					source,
					data: index_trips(await (await fetch(`/api/v1/trips/${source}`)).json())
				}))
			),
			Promise.all(
				default_sources.map(async (source) => ({
					source,
					data: index_stop_times(await (await fetch(`/api/v1/stop_times/${source}`)).json())
				}))
			),
			Promise.all(
				default_sources.map(async (source) => ({
					source,
					data: index_positions(await (await fetch(`/api/v1/positions/${source}`)).json())
				}))
			),
			Promise.all(
				default_sources.map(async (source) => ({
					source,
					data: index_alerts(await (await fetch(`/api/v1/alerts/${source}`)).json())
				}))
			)
		]);

	// TODO: fix type errors. I could use object.entries or something, but i need to test the performance implications of that first
	// maybe replace these with index functions similar to what rt initial values have
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
		// initial values for rt data and at param used for rt fetches
		at,
		initial_trips,
		initial_stop_times,
		initial_positions,
		initial_alerts
	};
};
