import { offline } from '$lib/stores';

export interface Stop {
	id: string;
	name: string;
	ada: boolean;
	notes?: string;
	borough: string;
	routes: Route[];
}

export interface Route {
	id: string;
	stop_type: StopType;
}

export enum StopType {
	FullTime = 0,
	PartTime = 1,
	LateNights = 2,
	RushHourOneDirection = 3,
	RushHourExtension = 4
}

export interface Trip {
	id: string;
	route_id: string;
	direction: Direction;
	assigned: boolean;
	created_at: string;
	stop_times: StopTime[];
	eta?: number;
}

export enum Direction {
	North = 1,
	South = 0
}

export interface StopTime {
	stop_id: string;
	arrival: string;
	departure: string;
}

type Fetch = typeof fetch;

export async function fetch_stops(fetch: Fetch): Promise<Stop[]> {
	const res = await fetch('/api/stops');
	// check if response is from service worker
	offline.set(res.headers.has('x-service-worker'));

	const data: Stop[] = await res.json();

	return data;
}

export async function fetch_trips(fetch: Fetch, stops: string[]): Promise<Trip[]> {
	const res = await fetch(`/api/trips?stop_ids=${stops.join(',')}`);
	// check if response is from service worker
	offline.set(res.headers.has('x-service-worker'));

	const data: Trip[] = await res.json();

	return data;
}

// export async function get_stops(ids: string[], times: boolean): Promise<Stop[]> {
// 	const res = await fetch(`/api/stops?ids=${ids.join(',')}&times=${times}`);
// 	// check if response is from service worker
// 	offline.set(res.headers.has('x-service-worker'));

// 	// TODO: error handling
// 	const data: Stop[] = await res.json();
// 	data.forEach((stop) => {
// 		stop.trips?.filter((trip) => trip.stop_times != null);
// 		// .map((trip) => {
// 		// 	// convert dates from string to Date
// 		// 	trip.created_at = new Date(trip.created_at);
// 		// 	trip.stop_times = trip.stop_times.map((stop_time) => {
// 		// 		stop_time.arrival = new Date(stop_time.arrival);
// 		// 		stop_time.departure = new Date(stop_time.departure);
// 		// 		retur
// 		// 	returnn stop_time;
// 		// 	}); trip;
// 		// });
// 	});
// 	// data.forEach((stop) => {
// 	// 	stop.
// 	// });

// 	return data;
// }
