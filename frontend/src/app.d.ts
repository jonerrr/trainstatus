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
			routes: Route[];
			stops: Stop[];
		}
		interface PageState<T extends string | number> {
			dialog_open: boolean;
			dialog_id: T;
			dialog_type: 'stop' | 'trip' | 'route' | '';
			data?: Stop | Trip | Route;
		}
		// interface Platform {}
	}
}

export {};
