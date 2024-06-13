import { writable } from 'svelte/store';
import { persisted } from 'svelte-persisted-store';
import type { Alert, Stop, StopTime, Trip } from '$lib/api';
import type { BusStop } from '$lib/bus_api';

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
// the amount of elements to show in pinned routes list before scroll is required
// export const pinned_routes_shown = persisted('pinned_routes_shown', 1);
export const location_status = persisted<LocationStatus>(
	'location_status',
	LocationStatus.NeverAsked
);

export const bus_mode = persisted('bus_mode', false);
export const bus_stops = writable<BusStop[]>([]);

// store length of each list element
export const item_heights = writable<{ [key: string | number]: number }>({});
