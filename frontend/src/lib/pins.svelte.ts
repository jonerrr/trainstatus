import type { Source } from '#lib/client/index.js';
import { LocalStorage } from '#lib/storage.svelte.js';

export type Pins = {
	[K in Source]: string[];
};
export const stopPins = new LocalStorage<Pins>('stopPins', {
	mta_subway: ['1'],
	mta_bus: ['400086'],
	njt_bus: []
});

export const routePins = new LocalStorage<Pins>('routePins', {
	mta_subway: ['4'],
	mta_bus: ['M15'],
	njt_bus: []
});

/** Trip pins carry their route; older trip-ID-only pins under `tripPins` are discarded. */
export const tripPins = new LocalStorage<Pins>('tripRoutePins', {
	mta_subway: [],
	mta_bus: [],
	njt_bus: []
});

export function tripPinKey(routeId: string, tripId: string): string {
	return JSON.stringify([routeId, tripId]);
}

export function parseTripPin(key: string): { routeId: string; tripId: string } | undefined {
	try {
		const [routeId, tripId] = JSON.parse(key);
		if (typeof routeId === 'string' && typeof tripId === 'string') return { routeId, tripId };
	} catch {
		// Not a route-qualified pin.
	}
	return undefined;
}
