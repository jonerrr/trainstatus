// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		interface PageState {
			dialog_open: boolean;
			dialog_id: string | number;
			// blank is not open
			dialog_type: 'stop' | 'trip' | 'route_alert' | 'route' | 'bus_stop' | 'bus_trip' | '';
			// list of bus routes to monitor
			monitor_routes?: string[];
		}
		// interface Platform {}
	}
}

export {};
