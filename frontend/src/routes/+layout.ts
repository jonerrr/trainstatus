import { alerts } from '$lib/alerts.svelte';
import { searchSchema } from '$lib/schemas';
import { default_sources } from '$lib/sources';
import { stop_times } from '$lib/stop_times.svelte';
import { trips } from '$lib/trips.svelte';

import type { Route, Source, Stop } from '@trainstatus/client';
import { validateSearchParams } from 'runed/kit';

import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch, url }) => {
	const { searchParams } = validateSearchParams(url, searchSchema);
	// print all search params
	const paramsObject = Object.fromEntries(searchParams.entries());
	console.dir({ paramsObject });

	const at = searchParams.get('at') ?? undefined;

	// Fetch stops and routes for all sources in parallel
	const [stopsResults, routesResults] = await Promise.all([
		Promise.all(
			default_sources.map(async (source) => ({
				source,
				data: (await fetch(`/api/v1/stops/${source}`).then((res) => res.json())) as Stop[]
			}))
		),
		Promise.all(
			default_sources.map(async (source) => ({
				source,
				data: (await fetch(`/api/v1/routes/${source}`).then((res) => res.json())) as Route[]
			}))
		)
		// TODO: add back realtime apis
	]);

	// Convert arrays to objects keyed by source
	const stops: App.PageData['stops'] = {};
	for (const { source, data } of stopsResults) {
		stops[source] = Object.fromEntries(data.map((stop) => [stop.id, stop]));
	}

	const routes: App.PageData['routes'] = {};
	for (const { source, data } of routesResults) {
		routes[source] = Object.fromEntries(data.map((route) => [route.id, route]));
	}

	return {
		stops,
		routes,
		at
	};
};
