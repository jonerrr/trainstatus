import type { Route, Stop } from '$lib/static';
import type { Trip } from '$lib/trips.svelte';

// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// maybe this should be maps
		interface PageData {
			routes: {
				[id: string]: Route;
			};
			// stops: Stop<'bus' | 'train'>[];
			stops: {
				[id: number]: Stop<'bus' | 'train'>;
			};
			bus_stops: Stop<'bus'>[];
			train_stops: Stop<'train'>[];
			// initial_promise: Promise<[void, void]>;
		}
		// <T extends string | number>
		interface PageState {
			// dialog_open: boolean;
			// dialog_id: T;
			// null is not open
			modal: 'stop' | 'trip' | 'route' | null;
			data?: Stop | Trip | Route;
		}
		// interface Platform {}
	}
}

export {};
