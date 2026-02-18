import type { Source } from '@trainstatus/client';
import { Context } from 'runed';

/**
 * Maps a Source to its corresponding resource instance.
 * Use this to build multi-source context types.
 *
 * @example
 * type MultiSourceTrips = SourceMap<TripResource>;
 * // Equivalent to: Record<Source, TripResource>
 */
export type SourceMap<T> = Record<Source, T>;

/**
 * Helper to create a multi-source context with proper typing.
 *
 * @example
 * export const trip_context = createMultiSourceContext<TripResource>('trips');
 */
export function createMultiSourceContext<T>(name: string): Context<SourceMap<T>> {
	return new Context<SourceMap<T>>(name);
}

/**
 * Type guard to check if a source exists in the source map
 */
// export function hasSource<T>(
// 	map: SourceMap<T>,
// 	source: Source
// ): map is SourceMap<T> & Record<typeof source, T> {
// 	return source in map;
// }

export const source_info = {
	// TODO: increase refresh interval
	// TODO: add icons
	mta_bus: {
		name: 'MTA Bus',
		icon: 'TODO',
		refresh_interval: {
			trips: 5000,
			stop_times: 5000,
			alerts: 5000
		},
		// this means that this source requires including specific routes in the query params
		// maybe find a better name for the param in the future
		monitor_routes: true
	},
	mta_subway: {
		name: 'MTA Subway',
		icon: 'TODO',
		refresh_interval: {
			trips: 5000,
			stop_times: 5000,
			alerts: 5000
		},
		monitor_routes: false
	}
} as const;

export const default_sources: Source[] = ['mta_bus', 'mta_subway'] as const;
// TODO: allow changing sources

type Fetcher<T> = (signal: AbortSignal) => Promise<T>;

interface LiveResourceOptions<T> {
	interval?: number;
	enabled?: boolean;
	debounce?: number;
	// TODO: maybe make this required
	initial_value?: T;
}
// TODO: probably take a default value for SSR
export class LiveResource<T> {
	// State
	value = $state<T | undefined>(undefined);
	error = $state<Error | null>(null);
	last_updated = $state<Date | null>(null);
	offline = $state(false);
	is_fetching = $state(false);

	// Configuration
	#interval_ms: number = $state(5000);
	#enabled: boolean = $state(true);
	#debounce_ms: number = $state(0);

	#fetcher: Fetcher<T>;
	#interval_timer: ReturnType<typeof setTimeout> | undefined;
	#debounce_timer: ReturnType<typeof setTimeout> | undefined;
	#abort_controller: AbortController | undefined;

	// Pending resolvers waiting for next successful fetch
	#pending_resolvers: Array<() => void> = [];

	constructor(fetcher: Fetcher<T>, options: LiveResourceOptions<T> = {}) {
		this.#fetcher = fetcher;
		if (options.interval) this.#interval_ms = options.interval;
		if (options.enabled !== undefined) this.#enabled = options.enabled;
		if (options.debounce) this.#debounce_ms = options.debounce;
		if (options.initial_value !== undefined) this.value = options.initial_value;

		$effect(() => {
			if (this.#enabled) {
				this.refresh();
				this.#startInterval();
			} else {
				this.#stopInterval();
				this.#abort_controller?.abort();
			}

			return () => {
				this.#stopInterval();
				this.#clearDebounce();
			};
		});
	}

	#startInterval() {
		this.#stopInterval();
		this.#interval_timer = setTimeout(() => {
			this.refresh();
		}, this.#interval_ms);
	}

	#stopInterval() {
		if (this.#interval_timer) {
			clearTimeout(this.#interval_timer);
			this.#interval_timer = undefined;
		}
	}

	#clearDebounce() {
		if (this.#debounce_timer) {
			clearTimeout(this.#debounce_timer);
			this.#debounce_timer = undefined;
		}
	}

	async refresh(immediate = false) {
		this.#clearDebounce();

		if (!immediate && this.#debounce_ms > 0) {
			this.#debounce_timer = setTimeout(() => {
				this.#executeFetch();
			}, this.#debounce_ms);
			return;
		}

		return this.#executeFetch();
	}

	/**
	 * Returns a promise that resolves after the next successful fetch completes.
	 * Triggers a debounced refresh so multiple calls within the debounce window
	 * are batched into a single request.
	 */
	next_refresh(): Promise<void> {
		const promise = new Promise<void>((resolve) => {
			this.#pending_resolvers.push(resolve);
		});
		this.refresh();
		return promise;
	}

	async #executeFetch() {
		if (this.is_fetching) return;

		this.is_fetching = true;
		this.#abort_controller = new AbortController();

		try {
			const data = await this.#fetcher(this.#abort_controller.signal);

			if (!this.#abort_controller.signal.aborted) {
				this.value = data;
				this.error = null;
				this.offline = false;
				this.last_updated = new Date();

				// Resolve all pending waiters
				const resolvers = this.#pending_resolvers.splice(0);
				for (const resolve of resolvers) resolve();
			}
		} catch (e) {
			if (e instanceof Error && e.name === 'AbortError') return;
			console.error('Resource fetch failed:', e);
			this.error = e as Error;
			if (e instanceof Error && e.message === 'Offline') this.offline = true;
			// Don't resolve pending on error - they'll wait for next successful fetch
		} finally {
			this.is_fetching = false;
			if (this.#enabled) this.#startInterval();
		}
	}
}
