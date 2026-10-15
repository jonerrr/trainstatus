import type { Source } from '#lib/client/index.js';

// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		interface PageData {
			selected_sources: Source[];
			at?: string;
		}
		interface PageState {
			// null is not open. Modals load their own data from these identities.
			modal:
				| null
				| { type: 'stop'; source: Source; id: string }
				| { type: 'route'; source: Source; id: string }
				| { type: 'trip'; source: Source; route_id: string; id: string };
			// used to determine if page.state update was forward or backwards
			index?: number;
		}
		// interface Platform {}
	}
}

export {};
