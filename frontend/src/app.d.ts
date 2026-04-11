import type { Route, Source, Stop, Trip } from '$lib/client';

// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		interface PageData {
			selected_sources: Source[];
			stops: Partial<Record<Source, Stop[]>>;
			routes: Partial<Record<Source, Route[]>>;
			stops_by_id: Partial<Record<Source, Record<string, Stop>>>;
			routes_by_id: Partial<Record<Source, Record<string, Route>>>;
			at?: string;
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
