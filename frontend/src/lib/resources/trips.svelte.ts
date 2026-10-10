import { SvelteDate, SvelteMap } from 'svelte/reactivity';

import type { Source } from '#lib/client/index.js';
import {
	createEntityResource,
	createMultiSourceContext,
	type TripResource,
	type TripResources,
	type TypedTrip
} from '#lib/resources/index.svelte.js';

export function index_trips<S extends Source>(data: TypedTrip<S>[]): TripResource<S> {
	return new SvelteMap(
		data.map((trip) => [
			trip.id,
			{
				...trip,
				created_at: new SvelteDate(trip.created_at),
				updated_at: new SvelteDate(trip.updated_at)
			}
		])
	);
}

export function createTripResource<S extends Source>(source: S) {
	return createEntityResource<TypedTrip<S>[], TripResource<S>>(
		source,
		'trips',
		index_trips<S>,
		new SvelteMap()
	);
}

export const trip_context = createMultiSourceContext<TripResources>();
