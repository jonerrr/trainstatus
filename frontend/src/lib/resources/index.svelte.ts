import { createContext } from 'svelte';

import type { SvelteMap } from 'svelte/reactivity';

import mta_bus_icon from '#lib/assets/mta_bus.webp?w=24;48;72&enhanced';
import mta_subway_icon from '#lib/assets/mta_subway.webp?w=24;48;72&enhanced';
import njt_bus_icon from '#lib/assets/njt_bus.webp?w=24;48;72&enhanced';
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
} from '#lib/client/index.js';
import { getCurrentTime } from '#lib/url_params.svelte.js';

import { LiveResource } from './liveResource.svelte';
import { requestData, resourceQuery, type DataKind } from './request';

export { LiveResource } from './liveResource.svelte';

export const source_info = {
	// TODO: increase refresh interval
	// TODO: use icons to differentiate between agencies
	// TODO: make it possible to disable refresh_interval for sources
	// TODO: make it possible to update refresh_interval time dynamically (e.g. if user is offline)
	mta_bus: {
		name: 'MTA Bus',
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
		name: 'MTA Subway',
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

export const all_sources = Object.keys(source_info) as Source[];

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

/** Factories own indexing; the shared owner captures time and handles all scheduling. */
export function createEntityResource<T, R>(
	source: Source,
	kind: Exclude<DataKind, 'stop_times' | 'trajectories'>,
	index: (data: T) => R,
	initial: R
) {
	const time = getCurrentTime();
	return new LiveResource<R>(
		(query, signal) =>
			requestData(query.url, async (response) => index(await response.json()), signal),
		initial,
		{
			query: () => resourceQuery(source, kind, time.value ?? null),
			interval: source_info[source].refresh_interval[kind],
			debounce: 500
		}
	);
}
