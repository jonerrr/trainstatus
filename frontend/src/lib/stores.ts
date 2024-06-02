import { writable } from 'svelte/store';
import { persisted } from 'svelte-persisted-store';
import type { Stop } from '$lib/api_new';

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

export const pinned_routes = persisted('pinned_routes', ['J', 'N', 'R']);
export const pinned_stops = persisted('pinned_stops', ['631', 'A27']);
