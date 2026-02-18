import { SvelteMap } from 'svelte/reactivity';

import { page } from '$app/state';

import { LiveResource } from '$lib/rt-resource.svelte';
import { source_info } from '$lib/sources';

import type { Source, Trip } from '@trainstatus/client';
import { Context } from 'runed';

type TripResource = SvelteMap<string, Trip>;

export function createTripResource(source: Source, params: { at?: number }) {
	const resource = new LiveResource<TripResource>(
		async (signal) => {
			console.log('updating trips');
			const query = new URLSearchParams();
			if (params.at) query.set('at', params.at.toString());

			const res = await fetch(`/api/v1/trips/${source}?${query}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error('Failed to fetch trips');

			const data: Trip[] = await res.json();
			//TODO: compare map and for loop performance
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
		},
		{
			interval: source_info[source].refresh_interval,
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

export const trip_context = new Context<ReturnType<typeof createTripResource>>('trips');

// export function createTripResource(source: Source, params: { at?: number }) {
// 	const tripResource = resource(
// 		() => params.at,
// 		async (at, prevAt, { signal }) => {
// 			const query = new URLSearchParams();
// 			if (at) query.set('at', at.toString());

// 			const res = await fetch(`/api/v1/trips/${source}?${query}`, { signal });

// 			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
// 			if (!res.ok) throw new Error('Failed to fetch trips');

// 			// TODO: use new trip type
// 			const data: Trip[] = await res.json();

// 			return new SvelteMap(
// 				data.map((trip) => [
// 					trip.id,
// 					{
// 						...trip,
// 						created_at: new Date(trip.created_at),
// 						updated_at: new Date(trip.updated_at)
// 					}
// 				])
// 			);
// 		},
// 		{
// 			initialValue: new SvelteMap(),
// 			throttle: 500 // TODO: maybe do debounce instead or increase time
// 		}
// 	);

// 	return tripResource;
// }
// TODO: Rename
// export const tripsNew = createTripResource('mta_subway');

// export interface Trip<T = TripData, R = never> {
// 	id: string;
// 	mta_id: string;
// 	route_id: string;
// 	vehicle_id: string;
// 	direction: TripDirection;
// 	data: T;
// 	created_at: Date;
// 	updated_at: Date;
// 	route: R;
// }

export type TripData = TrainTripData | BusTripData;

interface BaseTripData {
	stop_id?: number;
	// TODO: add more options to status here
	status: 'none' | 'incoming' | 'at_stop' | 'in_transit_to' | 'layover';
}

export interface TrainTripData extends BaseTripData {
	express: boolean;
	assigned: boolean;
}

export interface BusTripData extends BaseTripData {
	lat?: number;
	lon?: number;
	bearing?: number;
	passengers?: number;
	capacity?: number;
	deviation?: number;
}

// export enum TripStatus {
// 	None = 'none',
// 	// train statuses
// 	Incoming = 'incoming',
// 	AtStop = 'at_stop',
// 	InTransitTo = 'in_transit_to',
// 	// bus statuses
// 	Spooking = 'spooking',
// 	Layover = 'layover',
// 	NoProgress = 'no_progress'
// }

export enum TripDirection {
	North = 1,
	South = 0
}

type Fetch = typeof fetch;

export function createTrips() {
	// $state is redundant here
	let trips = $state(new SvelteMap<string, Trip>());

	// this returns true if there was an error (aka offline)
	async function update(fetch: Fetch, at?: string) {
		// // if (page.url.pathname)
		// // try {
		// const params = new URLSearchParams();
		// if (at) {
		// 	params.set('at', at);
		// }
		// // if (finished) {
		// // 	params.set('finished', 'true');
		// // 	// params.set('at', Math.floor((current_time.ms - 4 * 60 * 60 * 1000) / 1000).toString());
		// // }
		// const res = await fetch(`/api/v1/trips${params.size ? '?' + params.toString() : ''}`);
		// if (res.headers.has('x-sw-fallback')) {
		// 	throw new Error('Offline');
		// }
		// const data: Trip<TripData>[] = await res.json();
		// trips = new SvelteMap(
		// 	data.map((trip) => [
		// 		trip.id,
		// 		{
		// 			...trip,
		// 			created_at: new Date(trip.created_at),
		// 			updated_at: new Date(trip.updated_at)
		// 		}
		// 	])
		// );
	}

	return {
		update,
		get trips() {
			return trips;
		}
	};
}

export const trips = createTrips();

export const calculate_trip_height = () => 80;

// type guards for trips.
export const is_bus = (
	s: Stop<'bus' | 'train'>,
	t: Trip<TrainTripData | BusTripData>
): t is Trip<BusTripData> => {
	return s.route_type === 'bus';
};

export const is_train = (
	s: Stop<'bus' | 'train'>,
	t: Trip<TrainTripData | BusTripData>
): t is Trip<TrainTripData> => {
	return s.route_type === 'train';
};

// type guards for trip and route.
export const is_bus_route = (
	r: Route,
	t: Trip<TrainTripData | BusTripData, unknown>
): t is Trip<BusTripData> => {
	return r.route_type === 'bus';
};

export const is_train_route = (
	r: Route,
	t: Trip<TrainTripData | BusTripData, unknown>
): t is Trip<TrainTripData> => {
	return r.route_type === 'train';
};
