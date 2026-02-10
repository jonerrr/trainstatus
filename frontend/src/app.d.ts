import type { Route, Source, Stop, Trip } from '@trainstatus/client';

// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// maybe this should be maps
		interface PageData {
			// stops: {
			// 	source: Source;
			// 	data: Stop[];
			// }[];
			// routes: {
			// 	source: Source;
			// 	data: Route[];
			// }[];
			stops: Record<Source, Stop[]>;
			routes: Record<Source, Route[]>;

			stops_by_id: Record<Source, Record<string, Stop>>;
			routes_by_id: Record<Source, Record<string, Route>>;
			// trips: {
			// 	[id: string]: Trip;
			// };
			// routes: {
			// 	[id: string]: Route;
			// };
			// // stops: Stop<'bus' | 'train'>[];
			// stops: {
			// 	[id: number]: Stop;
			// };
			// bus_stops: Stop<'bus'>[];
			// train_stops: Stop<'train'>[];
			// initial current_time.value (can't set in layout load bc SSR)
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
			modal:
				| null
				| {
						type: 'stop';
						data: Stop;
						source: Source;
				  }
				| {
						type: 'trip';
						data: Trip;
						source: Source;
				  }
				| {
						type: 'route';
						data: Route;
						source: Source;
				  }
				| {
						type: 'settings';
				  };
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
