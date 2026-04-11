import { createContext } from 'svelte';

import type { SvelteMap } from 'svelte/reactivity';

import mta_bus_icon from '$lib/assets/mta_bus.webp';
import mta_subway_icon from '$lib/assets/mta_subway.webp';
// TODO: convert to webp
import njt_bus_icon from '$lib/assets/njt_bus.png';
import type {
	AlertData,
	ApiAlert,
	MtaBusPositionData,
	MtaSubwayPositionData,
	NjtBusPositionData,
	Source,
	StopTime,
	StopTimeData,
	Trip,
	TripData,
	VehiclePosition
} from '$lib/client';

export const source_info = {
	// TODO: increase refresh interval
	// TODO: use icons to differentiate between agencies
	// TODO: make it possible to disable refresh_interval for sources
	// TODO: make it possible to update refresh_interval time dynamically (e.g. if user is offline)
	mta_bus: {
		name: 'Bus',
		icon: mta_bus_icon,
		refresh_interval: {
			trips: 30_000,
			stop_times: 30_000,
			positions: 30_000,
			alerts: 45_000
		},
		// this means that this source requires including specific routes in the query params
		// maybe find a better name for the param in the future
		monitor_routes: true
	},
	mta_subway: {
		name: 'Subway',
		icon: mta_subway_icon,
		refresh_interval: {
			trips: 30_000,
			stop_times: 30_000,
			positions: 30_000,
			alerts: 45_000
			// TODO: maybe don't include subway positions since they don't contain really contain any useful info
		},
		monitor_routes: false
	},
	njt_bus: {
		name: 'NJT Bus',
		// TODO: update icon
		icon: njt_bus_icon,
		refresh_interval: {
			trips: 30_000,
			stop_times: 30_000,
			positions: 30_000,
			alerts: 45_000
		},
		monitor_routes: true
	}
} as const;

// =============================================================================
// SOURCE-SPECIFIC DATA MAPS
// Define the discriminated union mapping for each entity type
// =============================================================================

/** Position data discriminated by source */
export type SourcePositionDataMap = {
	mta_bus: MtaBusPositionData & { source: 'mta_bus' };
	mta_subway: MtaSubwayPositionData & { source: 'mta_subway' };
	njt_bus: NjtBusPositionData & { source: 'njt_bus' };
};

/** Trip data discriminated by source */
export type SourceTripDataMap = {
	mta_bus: Extract<TripData, { source: 'mta_bus' }>;
	mta_subway: Extract<TripData, { source: 'mta_subway' }>;
	njt_bus: Extract<TripData, { source: 'njt_bus' }>;
};

/** StopTime data discriminated by source */
export type SourceStopTimeDataMap = {
	mta_bus: Extract<StopTimeData, { source: 'mta_bus' }>;
	mta_subway: Extract<StopTimeData, { source: 'mta_subway' }>;
	njt_bus: Extract<StopTimeData, { source: 'njt_bus' }>;
};

/** Alert data discriminated by source */
export type SourceAlertDataMap = {
	mta_bus: Extract<AlertData, { source: 'mta_bus' }>;
	mta_subway: Extract<AlertData, { source: 'mta_subway' }>;
	njt_bus: Extract<AlertData, { source: 'njt_bus' }>;
};

// =============================================================================
// TYPED ENTITY HELPERS
// Creates a version of an entity with narrowed `data` field based on source
// =============================================================================

/**
 * Narrows an entity type's `data` field based on source.
 * @template Entity - The base entity type (e.g., VehiclePosition, Trip)
 * @template DataMap - The source-to-data mapping (e.g., SourcePositionDataMap)
 * @template S - The specific source
 */
export type TypedEntity<
	Entity extends { data: unknown },
	DataMap extends Record<Source, unknown>,
	S extends Source
> = Omit<Entity, 'data'> & { data: DataMap[S] };

// Convenience types for each entity
export type TypedVehiclePosition<S extends Source> = TypedEntity<
	VehiclePosition,
	SourcePositionDataMap,
	S
>;
export type TypedTrip<S extends Source> = TypedEntity<Trip, SourceTripDataMap, S>;
export type TypedStopTime<S extends Source> = TypedEntity<StopTime, SourceStopTimeDataMap, S>;
export type TypedAlert<S extends Source> = TypedEntity<ApiAlert, SourceAlertDataMap, S>;

// =============================================================================
// RESOURCE TYPES
// =============================================================================

/** A SvelteMap of entities keyed by ID */
export type EntityResource<T> = SvelteMap<string, T>;

/** Alert resource with route-indexed alerts */
export interface AlertResource<S extends Source> {
	alerts: TypedAlert<S>[];
	alerts_by_route: SvelteMap<string, TypedAlert<S>[]>;
}

/** Maps each source to its typed LiveResource */
export type SourceResources<T extends Record<Source, unknown>> = Partial<{
	[S in Source]: LiveResource<T[S]>;
}>;

// Convenience types for resource maps
export type PositionResource<S extends Source> = EntityResource<TypedVehiclePosition<S>>;
export type TripResource<S extends Source> = EntityResource<TypedTrip<S>>;

// StopTimes are indexed by both trip_id and stop_id
export interface StopTimeResource<S extends Source> {
	by_trip_id: SvelteMap<string, TypedStopTime<S>[]>;
	by_stop_id: SvelteMap<string, TypedStopTime<S>[]>;
}

export type PositionResources = SourceResources<{ [S in Source]: PositionResource<S> }>;
export type TripResources = SourceResources<{ [S in Source]: TripResource<S> }>;
export type AlertResources = SourceResources<{ [S in Source]: AlertResource<S> }>;

// =============================================================================
// MULTI-SOURCE CONTEXT
// =============================================================================

export type SourceMap<T> = Partial<Record<Source, T>>;

/**
 * Creates a typed multi-source context with a `getSource` helper
 * that properly narrows types based on the source parameter.
 */
export function createMultiSourceContext<ResourceMap extends SourceMap<unknown>>() {
	const [get, set] = createContext<ResourceMap>();

	function getSource<S extends Source>(source: S): ResourceMap[S] {
		const all = get();
		return all[source];
	}

	return { get, set, getSource };
}

type Fetcher<T> = (signal: AbortSignal) => Promise<T>;

export type ResourceStatus = 'initial' | 'fetching' | 'ready' | 'error';

interface LiveResourceOptions {
	interval?: number;
	enabled?: boolean;
	debounce?: number;
}

export class LiveResource<T> {
	current: T = $state() as T;
	status: ResourceStatus = $state<ResourceStatus>('initial');
	error: Error | null = $state(null);

	#interval_ms: number = $state(5000);
	#enabled: boolean = $state(true);
	#debounce_ms: number = $state(0);

	#fetcher: Fetcher<T>;
	#interval_timer: ReturnType<typeof setTimeout> | undefined;
	#debounce_timer: ReturnType<typeof setTimeout> | undefined;
	#abort_controller: AbortController | undefined;

	#pending_resolvers: Array<() => void> = [];
	#ready_resolvers: Array<(value: T) => void> = [];

	constructor(fetcher: Fetcher<T>, initial_value: T, options: LiveResourceOptions = {}) {
		this.#fetcher = fetcher;
		this.current = initial_value;
		if (options.interval) this.#interval_ms = options.interval;
		if (options.enabled !== undefined) this.#enabled = options.enabled;
		if (options.debounce) this.#debounce_ms = options.debounce;

		$effect(() => {
			if (this.#enabled) {
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
		const delay = this.status === 'initial' ? 0 : this.#interval_ms;
		this.#interval_timer = setTimeout(() => {
			this.refresh();
		}, delay);
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

	/**
	 * Returns a promise that resolves with the current value once the resource
	 * has successfully fetched at least once. Resolves immediately if already ready.
	 */
	whenReady(): Promise<T> {
		if (this.status === 'ready') return Promise.resolve(this.current);
		return new Promise<T>((resolve) => {
			this.#ready_resolvers.push(resolve);
		});
	}

	async #executeFetch() {
		if (this.status === 'fetching') return;

		this.status = 'fetching';
		this.#abort_controller = new AbortController();

		try {
			const data = await this.#fetcher(this.#abort_controller.signal);

			if (!this.#abort_controller.signal.aborted) {
				this.current = data;
				this.error = null;
				this.status = 'ready';

				const resolvers = this.#pending_resolvers.splice(0);
				for (const resolve of resolvers) resolve();

				const ready_resolvers = this.#ready_resolvers.splice(0);
				for (const resolve of ready_resolvers) resolve(data);
			}
		} catch (e) {
			if (e instanceof Error && e.name === 'AbortError') {
				// Status cleared in `finally` — avoids leaving `fetching`, which would block
				// subsequent `#executeFetch` calls at the guard below.
			} else {
				console.error('Resource fetch failed:', e);
				this.error = e as Error;
				this.status = 'error';
			}
		} finally {
			// Abort paths (catch AbortError, or fetch resolved after abort) never assign
			// `status`, so we would stay `fetching` forever and block future refreshes.
			if (this.status === 'fetching') this.status = 'ready';

			if (this.#enabled) this.#startInterval();
		}
	}
}
