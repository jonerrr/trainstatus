import { SvelteMap } from 'svelte/reactivity';

import type { Source } from '$lib/client';
import {
	LiveResource,
	type TripResource,
	type TripResources,
	type TypedTrip,
	createMultiSourceContext,
	source_info
} from '$lib/resources/index.svelte';
import { current_time } from '$lib/url_params.svelte';

export function index_trips<S extends Source>(data: TypedTrip<S>[]): TripResource<S> {
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

export function createTripResource<S extends Source>(source: S) {
	const resource = new LiveResource<TripResource<S>>(
		async (signal) => {
			console.log(`updating ${source} trips`);

			const at = current_time.value;
			const query_params = at ? `?at=${at}` : '';
			const res = await fetch(`/api/v1/trips/${source}${query_params}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error('Failed to fetch trips');

			const data: TypedTrip<S>[] = await res.json();
			return index_trips<S>(data);
		},
		new SvelteMap(),
		{
			interval: source_info[source].refresh_interval.trips,
			debounce: 500
		}
	);

	let prev_time = current_time.value;
	$effect(() => {
		const val = current_time.value;
		if (val !== prev_time) {
			prev_time = val;
			resource.refresh();
		}
	});

	return resource;
}

export const trip_context = createMultiSourceContext<TripResources>();
