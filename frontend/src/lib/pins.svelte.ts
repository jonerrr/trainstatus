import { LocalStorage } from '$lib/storage.svelte';

import type { Source } from '@trainstatus/client';

export type Pins = {
	[K in Source]: string[];
};
// TODO: maybe have separate LocalStorage for each source
export const stop_pins = new LocalStorage<Pins>('stop_pins', {
	mta_subway: ['106'],
	mta_bus: ['400086'],
	njt_bus: []
	// lirr: [],
	// metro_north: []
});

export const route_pins = new LocalStorage<Pins>('route_pins', {
	mta_subway: ['4'],
	mta_bus: ['M15'],
	njt_bus: []
	// lirr: [],
	// metro_north: []
});

export const trip_pins = new LocalStorage<Pins>('trip_pins', {
	mta_subway: [],
	mta_bus: [],
	njt_bus: []
	// lirr: [],
	// metro_north: []
});
