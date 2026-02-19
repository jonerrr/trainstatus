import { SvelteMap } from 'svelte/reactivity';

import { LiveResource, createMultiSourceContext, source_info } from '$lib/sources/index.svelte';

import type { Source, Trip } from '@trainstatus/client';

export type TripResource = SvelteMap<string, Trip>;

//TODO: compare map and for loop performance
export function index_trips(data: Trip[]): TripResource {
	return new SvelteMap(
		data.map((trip) => [
			trip.id,
			{
				...trip,
				created_at: new Date(trip.created_at),
				updated_at: new Date(trip.updated_at)
			}
		])
	);
}

export function createTripResource(
	source: Source,
	params: { at?: number },
	initial_value: TripResource
) {
	const resource = new LiveResource<TripResource>(
		async (signal) => {
			console.log('updating trips');
			const query = new URLSearchParams();
			if (params.at) query.set('at', params.at.toString());

			const res = await fetch(`/api/v1/trips/${source}?${query}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error('Failed to fetch trips');

			const data: Trip[] = await res.json();
			return index_trips(data);
			// return new SvelteMap(
			// 	data.map((trip) => [
			// 		trip.id,
			// 		{
			// 			...trip,
			// 			created_at: new Date(trip.created_at),
			// 			updated_at: new Date(trip.updated_at)
			// 		}
			// 	])
			// );
		},
		{
			initial_value,
			interval: source_info[source].refresh_interval.trips,
			debounce: 500 // TODO: increase time
		}
	);

	$effect(() => {
		if (params.at !== undefined) {
			resource.refresh();
		}
	});

	return resource;
}

export const trip_context =
	createMultiSourceContext<ReturnType<typeof createTripResource>>('trips');

export const calculate_trip_height = () => 80;
