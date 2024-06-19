import type { Writable } from 'svelte/store';
import { offline } from '$lib/stores';

type Fetch = typeof fetch;

// monitored routes will change on geolocate, when searching, and clicking through dialogs
export async function update_bus_data(
	fetch: Fetch,
	trip_store: Writable<BusTrip[]>,
	stop_time_store: Writable<BusStopTime[]>,
	routes: string[]
) {
	try {
		const route_l = routes.join(',');
		const [tripsResponse, stopTimesResponse] = await Promise.all([
			fetch(`/api/bus/trips?route_ids=${route_l}`),
			fetch(`/api/bus/routes/arrivals?route_ids=${route_l}`)
		]);

		if (
			tripsResponse.headers.has('x-service-worker') ||
			stopTimesResponse.headers.has('x-service-worker')
		)
			offline.set(true);
		else offline.set(false);

		const [trips, stopTimes] = await Promise.all([
			tripsResponse.json().then((data: BusTrip[]) => {
				return data.map((t: BusTrip) => {
					return {
						...t,
						created_at: new Date(t.created_at)
					};
				});
			}),
			stopTimesResponse.json().then((data: BusStopTime[]) => {
				return data.map((st: BusStopTime) => {
					return {
						...st,
						arrival: new Date(st.arrival),
						departure: new Date(st.departure)
					};
				});
			})
		]);

		trip_store.set(trips);
		stop_time_store.set(stopTimes);
	} catch (e) {
		console.error(e);
		offline.set(true);
	}
}

export interface BusRoute {
	id: string;
	long_name: string;
	short_name: string;
	color: string;
	shuttle: boolean;
}

export interface BusStop {
	id: number;
	name: string;
	direction: string;
	lat: number;
	lon: number;
	routes: BusRouteStop[];
}

export interface BusRouteStop {
	id: string;
	direction: 0 | 1;
	headsign: string;
}

export interface BusTrip {
	id: string;
	route_id: string;
	direction: 0 | 1;
	vehicle_id: number;
	deviation: number | null;
	created_at: Date;
	lat: number | null;
	lon: number | null;
	progress_status: 'layover' | 'spooking' | null;
	passengers: number | null;
	capacity: number | null;
	stop_id: number | null;
	headsign: string;
}

export interface BusStopTime {
	trip_id: string;
	stop_id: number;
	arrival: Date;
	departure: Date;
	stop_sequence: number;
	route_id: string;
	eta?: number;
}
