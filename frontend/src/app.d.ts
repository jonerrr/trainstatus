import type { AlertResource } from '$lib/resources/alerts.svelte';
import type { StopTimesResource } from '$lib/resources/stop_times.svelte';
import type { TripResource } from '$lib/resources/trips.svelte';

import type { ApiAlert, Route, Source, Stop, StopTime, Trip } from '@trainstatus/client';

interface RealtimeInitialValue<T> {
	source: Source;
	data: T;
}

// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// maybe this should be maps
		interface PageData {
			stops: Record<Source, Stop[]>;
			routes: Record<Source, Route[]>;
			stops_by_id: Record<Source, Record<string, Stop>>;
			routes_by_id: Record<Source, Record<string, Route>>;
			// Initial realtime values as SourceMaps
			initial_trips: RealtimeInitialValue<TripResource>[];
			initial_stop_times: RealtimeInitialValue<StopTimesResource>[];
			initial_positions: RealtimeInitialValue<PositionResource>[];
			initial_alerts: RealtimeInitialValue<AlertResource>[];
			// Current time param for RT fetches
			at?: string;
			// used to keep track of the current monitored
			// current_monitored_routes: string[];
			// initial_promise: Promise<[void, void]>;
		}
		// <T extends string | number>
		interface PageState {
			// dialog_open: boolean;
			// dialog_id: T;
			// null is not open
			// type: 'stop' | 'trip' | 'route' | 'settings' | null;
			modal:
				| null
				| (Stop & { type: 'stop' })
				| (Trip & { type: 'trip' })
				| (Route & { type: 'route' })
				| { type: 'settings' };
			// TODO: require that if modal isn't null, data must be provided
			// modal: 'stop' | 'trip' | 'route' | 'settings' | null;
			// data?: Stop | Trip | Route;
			// source?: Source; // maybe don't store source in page state
			// time used for api requests.
			// at?: number;
		}
		// interface Platform {}
	}
}

export {};
