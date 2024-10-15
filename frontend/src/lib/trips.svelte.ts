import { SvelteMap } from 'svelte/reactivity';
import type { Route, Stop } from './static';

export interface Trip<T extends TripData, R = never> {
	id: string;
	mta_id: string;
	route_id: string;
	status: 'none' | 'incoming' | 'at_stop' | 'in_transit_to' | 'layover';
	stop_id?: number;
	vehicle_id: string;
	direction: TripDirection;
	data: T;
	created_at: Date;
	updated_at: Date;
	route: R;
}

type TripData = TrainTripData | BusTripData;

export interface TrainTripData {
	express: boolean;
	assigned: boolean;
}

export interface BusTripData {
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
	const trips = $state(new SvelteMap<string, Trip<TripData>>());

	// this returns true if there was an error (aka offline)
	async function update(fetch: Fetch) {
		try {
			const data: Trip<TripData>[] = await (await fetch('/api/v1/trips')).json();
			trips.clear();

			data.forEach((trip) => {
				trips.set(trip.id, {
					...trip,
					created_at: new Date(trip.created_at),
					updated_at: new Date(trip.updated_at)
				});
			});

			return false;
		} catch (e) {
			console.error(e);
			return true;
		}
		// .then((res) => res.json())
		// .then(
		// 	(data) =>
		// 		// convert dates from strings to Date objects and put into map
		// (trips = new Map(
		// 	data.map((trip: Trip<TripData>) => [
		// 		trip.id,
		// 		{
		// 			...trip,
		// 			created_at: new Date(trip.created_at),
		// 			updated_at: new Date(trip.updated_at)
		// 		}
		// 	])
		// ))
		// (trips = data.map((trip: Trip<TripData>) => ({
		// 	...trip,
		// 	created_at: new Date(trip.created_at),
		// 	updated_at: new Date(trip.updated_at)
		// })))
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

// type guards for trips.
export const is_bus = (
	s: Stop<'bus' | 'train'>,
	t: Trip<TrainTripData | BusTripData>
): t is Trip<BusTripData> => {
	return s.type === 'bus';
};

export const is_train = (
	s: Stop<'bus' | 'train'>,
	t: Trip<TrainTripData | BusTripData>
): t is Trip<TrainTripData> => {
	return s.type === 'train';
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
