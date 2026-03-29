import type { Source } from '$lib/client';
import { LocalStorage } from '$lib/storage.svelte';

export type Pins = {
	[K in Source]: string[];
};
export const stop_pins = new LocalStorage<Pins>('stopPins', {
	mta_subway: ['106'],
	mta_bus: ['400086'],
	njt_bus: []
	// lirr: [],
	// metro_north: []
});

export const route_pins = new LocalStorage<Pins>('routePins', {
	mta_subway: ['4'],
	mta_bus: ['M15'],
	njt_bus: []
	// lirr: [],
	// metro_north: []
});

export const trip_pins = new LocalStorage<Pins>('tripPins', {
	mta_subway: [],
	mta_bus: [],
	njt_bus: []
	// lirr: [],
	// metro_north: []
});
