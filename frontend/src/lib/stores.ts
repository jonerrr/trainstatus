import { writable } from 'svelte/store';
import { persisted } from 'svelte-persisted-store';

export const loading = writable(false);
export const offline = writable(false);

export const pinned_routes = persisted('pinned_routes', ['4', 'A']);
export const pinned_stops = persisted('pinned_stops', ['631', 'A27']);
