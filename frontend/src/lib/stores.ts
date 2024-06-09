import { writable } from 'svelte/store';
import { persisted } from 'svelte-persisted-store';
import type { Alert, Stop, StopTime, Trip } from '$lib/api';

export enum LocationStatus {
	NeverAsked,
	Loading,
	Denied,
	Granted
}
export const location_status = persisted<LocationStatus>(
	'location_status',
	LocationStatus.NeverAsked
);

export const loading = writable(false);
export const offline = writable(false);
export const stops = writable<Stop[]>([]);
export const trips = writable<Trip[]>([]);
export const stop_times = writable<StopTime[]>([]);
export const alerts = writable<Alert[]>([]);

export const pinned_routes = persisted('pinned_routes', ['J']);
export const pinned_stops = persisted('pinned_stops', ['631']);
