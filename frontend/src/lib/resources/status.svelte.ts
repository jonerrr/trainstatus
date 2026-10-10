import { createContext } from 'svelte';

import { SvelteSet } from 'svelte/reactivity';

export interface UpdateEntry {
	readonly active: boolean;
	readonly error: Error | null;
	refresh(immediate?: boolean): Promise<unknown>;
}
export class UpdateStatus {
	entries = new SvelteSet<UpdateEntry>();
	register(entry: UpdateEntry) {
		this.entries.add(entry);
		return () => this.entries.delete(entry);
	}
	get failed(): boolean {
		return [...this.entries].some((entry) => entry.active && !!entry.error);
	}
	async retry(all = false) {
		await Promise.allSettled(
			[...this.entries.values()]
				.filter((entry) => entry.active && (all || entry.error))
				.map((entry) => entry.refresh(true))
		);
	}
}
export const [getUpdateStatus, setUpdateStatus] = createContext<UpdateStatus>();
