import type { Source } from '#lib/client/index.js';

export interface ResourceSnapshot<T> {
	value: T | undefined;
	loading: boolean;
	/** Initial-load failure. Refresh failures retain a loaded value. */
	error: Error | null;
}

/** Bind once during initialization; reading current only observes native state. */
export interface Resource<T> {
	readonly current: T;
}

export type Kind = 'catalog' | 'alerts' | 'service';

export class Entry<T> {
	state = $state.raw<{ data: T | undefined; error: Error | null }>({
		data: undefined,
		error: null
	});
	readers = 0;
	eager = false;
	in_flight = false;
	fetched_at: number | null = null;
	source_updated_at: number | null = null;
	attempted_at: number | null = null;
	failures = 0;
	used_at = 0;
	/** Invalidates a request if another publisher installs newer data. */
	revision = 0;

	constructor(
		readonly source: Source,
		readonly at: number | undefined,
		readonly route_id: string | null,
		readonly kind: Kind = 'service'
	) {}

	get data() {
		return this.state.data;
	}
	get error() {
		return this.state.error;
	}
	get active() {
		return this.readers > 0;
	}
	get current(): ResourceSnapshot<T> {
		const { data, error } = this.state;
		return {
			value: data,
			loading: data === undefined && !error,
			error: data === undefined ? error : null
		};
	}

	install(data: T, now: number) {
		this.revision++;
		this.fetched_at = now;
		this.failures = 0;
		this.state = { data, error: null };
	}

	fail(error: Error) {
		this.failures++;
		this.state = { data: this.data, error };
	}

	reset() {
		if (this.data !== undefined || !this.error) return;
		this.state = { data: undefined, error: null };
	}
}
