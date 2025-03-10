import { SvelteMap } from 'svelte/reactivity';
import type { Route, Stop } from './static';

export interface Trip<T = TripData, R = never> {
	id: string;
	mta_id: string;
	route_id: string;
	vehicle_id: string;
	direction: TripDirection;
	data: T;
	created_at: Date;
	updated_at: Date;
	route: R;
}

export type TripData = TrainTripData | BusTripData;

interface BaseTripData {
	stop_id?: number;
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
	let trips = $state(new SvelteMap<string, Trip<TripData>>());

	// this returns true if there was an error (aka offline)
	async function update(fetch: Fetch, at?: string) {
		// try {
		const res = await fetch(`/api/v1/trips${at ? `?at=${at}` : ''}`);
		if (res.headers.has('x-sw-fallback')) {
			throw new Error('Offline');
		}

		const data: Trip<TripData>[] = await res.json();

		trips = new SvelteMap(
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
