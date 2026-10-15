import type { Source } from '#lib/client/index.js';
import { LocalStorage } from '#lib/storage.svelte.js';

export type Pins = {
	[K in Source]: string[];
};
export const stop_pins = new LocalStorage<Pins>('stopPins', {
	mta_subway: ['1'],
	mta_bus: ['400086'],
	njt_bus: []
});

export const route_pins = new LocalStorage<Pins>('routePins', {
	mta_subway: ['4'],
	mta_bus: ['M15'],
	njt_bus: []
});

/** Trip pins carry their route; older trip-ID-only pins under `tripPins` are discarded. */
export const trip_pins = new LocalStorage<Pins>('tripRoutePins', {
	mta_subway: [],
	mta_bus: [],
	njt_bus: []
});

export function trip_pin_key(route_id: string, trip_id: string): string {
	return JSON.stringify([route_id, trip_id]);
}

export function parse_trip_pin(key: string): { route_id: string; trip_id: string } | undefined {
	try {
		const [route_id, trip_id] = JSON.parse(key);
		if (typeof route_id === 'string' && typeof trip_id === 'string') return { route_id, trip_id };
	} catch {
		// Not a route-qualified pin.
	}
	return undefined;
}
