import { SvelteMap, SvelteSet } from 'svelte/reactivity';

import { LiveResource } from '$lib/rt-resource.svelte';
import { source_info } from '$lib/sources';

import type { Source, StopTime } from '@trainstatus/client';
import { Context } from 'runed';

export interface StopTimeIndex {
	by_trip_id: SvelteMap<string, StopTime[]>;
	by_stop_id: SvelteMap<string, StopTime[]>;
}

function index_stop_times(data: StopTime[]): StopTimeIndex {
	const by_trip_id = new SvelteMap<string, StopTime[]>();
	const by_stop_id = new SvelteMap<string, StopTime[]>();

	for (const st of data) {
		// if (typeof st.arrival === 'string')
		st.arrival = new Date(st.arrival);
		// if (typeof st.departure === 'string')
		st.departure = new Date(st.departure);

		if (!by_trip_id.has(st.trip_id)) by_trip_id.set(st.trip_id, []);
		if (!by_stop_id.has(st.stop_id)) by_stop_id.set(st.stop_id, []);

		by_trip_id.get(st.trip_id)!.push(st);
		by_stop_id.get(st.stop_id)!.push(st);
	}

	return { by_trip_id, by_stop_id };
}

const EMPTY_INDEX: StopTimeIndex = {
	by_trip_id: new SvelteMap(),
	by_stop_id: new SvelteMap()
};

export function createStopTimesResource(source: Source, params: { at?: number } = {}) {
	// SvelteSet is inherently reactive — the fetcher closes over it and reads
	// the current snapshot on every invocation, so no $state() wrapper needed.
	const monitored_routes = new SvelteSet<string>();

	const live = new LiveResource<StopTimeIndex>(
		async (signal) => {
			console.log('updating stop times');
			const routes = [...monitored_routes];

			// Sources that require explicit routes (e.g. bus) return empty until
			// at least one route is monitored, avoiding a useless all-routes request.
			if (source_info[source].monitor_routes && routes.length === 0) {
				return EMPTY_INDEX;
			}

			const query = new URLSearchParams();
			if (params.at) query.set('at', params.at.toString());
			// TODO: encodeURIComponent for route ids that contain special chars (e.g. "+")
			if (routes.length) query.set('route_ids', routes.join(','));

			const res = await fetch(`/api/v1/stop_times/${source}?${query}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error(`Failed to fetch stop times: ${res.status}`);

			const data: StopTime[] = await res.json();
			return index_stop_times(data);
		},
		{
			interval: source_info[source].refresh_interval,
			debounce: 500
		}
	);

	return {
		get value() {
			return live.value;
		},
		get error() {
			return live.error;
		},
		get is_fetching() {
			return live.is_fetching;
		},
		get last_updated() {
			return live.last_updated;
		},
		get offline() {
			return live.offline;
		},

		get by_trip_id() {
			return live.value?.by_trip_id ?? EMPTY_INDEX.by_trip_id;
		},
		get by_stop_id() {
			return live.value?.by_stop_id ?? EMPTY_INDEX.by_stop_id;
		},

		get monitored_routes(): ReadonlySet<string> {
			return monitored_routes;
		},

		/**
		 * Add a route to monitor. Debounces the underlying fetch so multiple calls
		 * within the debounce window are coalesced into a single request.
		 *
		 * @returns A promise that resolves once the fetch that includes this route
		 *   completes successfully. If the route was already monitored and data is
		 *   available, resolves immediately.
		 */
		add_route(route_id: string): Promise<void> {
			if (monitored_routes.has(route_id)) {
				// Already tracked — resolve right away if we have data, otherwise
				// wait for the next successful fetch (e.g. initial load still pending).
				return live.value ? Promise.resolve() : live.next_refresh();
			}
			monitored_routes.add(route_id);
			return live.next_refresh();
		},

		/**
		 * Stop monitoring a route. The change takes effect on the next refresh cycle;
		 * no immediate re-fetch is triggered.
		 */
		remove_route(route_id: string): void {
			monitored_routes.delete(route_id);
		},

		refresh(): Promise<void> | undefined {
			return live.refresh();
		}
	};
}

export type StopTimesResource = ReturnType<typeof createStopTimesResource>;

export const stop_times_context = new Context<StopTimesResource>('stop_times');

// TODO: remove when finished refactoring
export const monitored_bus_routes = new SvelteSet<string>();
