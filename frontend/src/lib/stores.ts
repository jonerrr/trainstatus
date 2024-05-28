import { writable } from 'svelte/store';
import { persisted } from 'svelte-persisted-store';
import type { Stop } from '$lib/api';

export const loading = writable(false);
export const offline = writable(false);
export const stops = writable<Stop[]>([]);

// '4', 'A', 'J'
export const pinned_routes = persisted('pinned_routes', ['J', 'Q', 'N', 'A', 'H']);
export const pinned_stops = persisted('pinned_stops', ['631', 'A27']);
