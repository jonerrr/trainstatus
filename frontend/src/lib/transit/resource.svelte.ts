import type { Source } from '#lib/client/index.js';

export type Read<T> =
	| { state: 'idle' }
	| { state: 'loading' }
	| { state: 'missing' }
	| { state: 'ready'; value: T }
	| { state: 'failed'; error: Error };

type Snapshot<T> = Extract<Read<T>, { state: 'loading' | 'missing' | 'ready' }>;

/** Bind once during initialization; reading current only observes native state. */
export interface Resource<T> {
	readonly current: T;
}

export type Kind = 'catalog' | 'alerts' | 'service';

export function toError(error: unknown): Error {
	return error instanceof Error ? error : new Error(String(error));
}

export class Entry<T> {
	state = $state.raw<{ snapshot: Snapshot<T>; error: Error | null; fatal: boolean }>({
		snapshot: { state: 'loading' },
		error: null,
		fatal: false
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

	get snapshot() {
		return this.state.snapshot;
	}
	get data() {
		return this.snapshot.state === 'ready' ? this.snapshot.value : undefined;
	}
	get error() {
		return this.state.error;
	}
	get fatal() {
		return this.state.fatal;
	}
	get active() {
		return this.readers > 0;
	}
	get current(): Read<T> {
		const { snapshot, error, fatal } = this.state;
		if (error && (fatal || snapshot.state !== 'ready')) return { state: 'failed', error };
		return snapshot;
	}

	install(value: T, now: number) {
		this.#publish({ state: 'ready', value }, now);
	}
	markMissing(now: number) {
		this.#publish({ state: 'missing' }, now);
	}
	#publish(snapshot: Snapshot<T>, now: number) {
		this.revision++;
		this.fetched_at = now;
		this.failures = 0;
		this.state = { snapshot, error: null, fatal: false };
	}

	fail(error: Error, fatal = false) {
		this.failures++;
		this.state = { snapshot: this.snapshot, error, fatal };
	}

	reset() {
		if (!this.error || (this.snapshot.state === 'ready' && !this.fatal)) return;
		this.state = { snapshot: { state: 'loading' }, error: null, fatal: false };
	}
}
