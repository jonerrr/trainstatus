import { writable } from 'svelte/store';
import { persisted } from 'svelte-persisted-store';
import type { Alert, Stop, StopTime, Trip } from '$lib/api';
import type { BusRoute, BusStop, BusStopTime, BusTrip } from '$lib/bus_api';

export enum LocationStatus {
	NeverAsked,
	Loading,
	Denied,
	Granted
}

export const loading = writable(false);
export const offline = writable(false);
export const stops = writable<Stop[]>([]);
export const trips = writable<Trip[]>([]);
export const stop_times = writable<StopTime[]>([]);
export const alerts = writable<Alert[]>([]);

export const pinned_routes = persisted('pinned_routes', ['J']);
// which of the pin lists have accordion open
export const pinned_stops = persisted('pinned_stops', ['220']);
export const pinned_trips = persisted<string[]>('pinned_trips', []);
// the amount of elements to show in pinned routes list before scroll is required
// export const pinned_routes_shown = persisted('pinned_routes_shown', 1);
export const location_status = persisted<LocationStatus>(
	'location_status',
	LocationStatus.NeverAsked
);
// store which tab user was last on in stop dialog
export const stop_direction = persisted<'northbound' | 'southbound'>(
	'stop_direction',
	'northbound'
);

// list of bus routes to fetch from api bc we can't fetch all at once
export const monitored_routes = writable<string[]>([]);

export const bus_routes = writable<BusRoute[]>([]);
export const bus_stops = writable<BusStop[]>([]);
export const bus_trips = writable<BusTrip[]>([]);
export const bus_stop_times = writable<BusStopTime[]>([]);

// export const pinned_bus_routes = persisted('pinned_bus_routes', ['B44']);
export const pinned_bus_stops = persisted<number[]>('pinned_bus_stops', []);

// export interface PinnedBusTrip {
// 	id: string;
// 	routes: string[];
// }
// format is route id _ trip id
export const pinned_bus_trips = persisted<string[]>('pinned_bus_trips', []);

// stores the time of data they want, null = current time
export const data_at = writable<Date | null>(null);

// for train, stop id is string but for bus its number
// type StopIdType<T> = T extends StopTime[] ? string : T extends BusStopTime[] ? number : never;

// export function get_stop_times<T extends StopTime[] | BusStopTime[]>(
// 	store: Writable<T>,
// 	stop_id: StopIdType<T>
// ) {}
