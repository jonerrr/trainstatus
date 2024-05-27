import { writable } from 'svelte/store';
import { persisted } from 'svelte-persisted-store';
import type { Stop } from '$lib/api';

export const loading = writable(false);
export const offline = writable(false);
export const stops = writable<Stop[]>([]);

export const pinned_routes = persisted('pinned_routes', ['4', 'A', 'F']);
export const pinned_stops = persisted('pinned_stops', ['631', 'A27', 'D26', '901', 'R20']);
