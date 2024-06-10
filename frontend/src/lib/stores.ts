import { writable } from 'svelte/store';
import { persisted } from 'svelte-persisted-store';
import type { Alert, Stop, StopTime, Trip } from '$lib/api';

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
// export const expanded_pinned = persisted('expanded_pinned', ['routes']);
export const pinned_stops = persisted('pinned_stops', ['631']);
export const location_status = persisted<LocationStatus>(
	'location_status',
	LocationStatus.NeverAsked
);
export const bus_mode = persisted('bus_mode', false);
