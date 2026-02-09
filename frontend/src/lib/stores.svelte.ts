import type { Source } from '@trainstatus/client';
import { PersistedState } from 'runed';

export type Pins = {
	[K in Source]: string[];
};

export const stop_pins = new PersistedState<Pins>('stop_pins', {
	mta_subway: ['106'],
	mta_bus: ['400086']
	// lirr: [],
	// metro_north: []
});

export const route_pins = new PersistedState<Pins>('route_pins', {
	mta_subway: ['4'],
	mta_bus: ['M15']
	// lirr: [],
	// metro_north: []
});
