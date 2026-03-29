import type { ApiAlert, Route, Source, Stop, StopTime, Trip } from '$lib/client';
import type { AlertResource } from '$lib/resources/alerts.svelte';
import type { PositionResource } from '$lib/resources/positions.svelte';
import type { StopTimesResource } from '$lib/resources/stop_times.svelte';
import type { TripResource } from '$lib/resources/trips.svelte';

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
			selected_sources: Source[];
			stops: Partial<Record<Source, Stop[]>>;
			routes: Partial<Record<Source, Route[]>>;
			stops_by_id: Partial<Record<Source, Record<string, Stop>>>;
			routes_by_id: Partial<Record<Source, Record<string, Route>>>;
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
				| (Route & { type: 'route' });
			// used to determine if page.state update was forward or backwards
			index?: number;
		}
		// interface Platform {}
	}
}

export {};
