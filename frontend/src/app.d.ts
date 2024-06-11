// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		interface PageState {
			dialog_open: boolean;
			dialog_id: string;
			// blank is not open
			dialog_type: 'stop' | 'trip' | 'route_alert' | 'route' | '';
		}
		// interface Platform {}
	}
}

export {};
