import { SvelteMap, SvelteSet } from 'svelte/reactivity';

import {
	LiveResource,
	type StopTimeResource,
	type TypedStopTime,
	createMultiSourceContext,
	source_info
} from '$lib/resources/index.svelte';
import { current_time } from '$lib/util.svelte';

import type { Source } from '@trainstatus/client';

export function index_stop_times<S extends Source>(data: TypedStopTime<S>[]): StopTimeResource<S> {
	const by_trip_id = new SvelteMap<string, TypedStopTime<S>[]>();
	const by_stop_id = new SvelteMap<string, TypedStopTime<S>[]>();

	for (const st of data) {
		const typed_st = {
			...st,
			arrival: new Date(st.arrival),
			departure: new Date(st.departure)
		} as TypedStopTime<S>;

		if (!by_trip_id.has(st.trip_id)) by_trip_id.set(st.trip_id, []);
		if (!by_stop_id.has(st.stop_id)) by_stop_id.set(st.stop_id, []);

		by_trip_id.get(st.trip_id)!.push(typed_st);
		by_stop_id.get(st.stop_id)!.push(typed_st);
	}

	return { by_trip_id, by_stop_id };
}

const EMPTY_INDEX: StopTimeResource<Source> = {
	by_trip_id: new SvelteMap(),
	by_stop_id: new SvelteMap()
};

export function createStopTimeResource<S extends Source>(
	source: S,
	initial_value: StopTimeResource<S>
) {
	const monitored_routes = new SvelteSet<string>();

	const resource = new LiveResource<StopTimeResource<S>>(
		async (signal) => {
			console.log(`updating ${source} stop times`);

			const routes = [...monitored_routes];

			// Sources that require explicit routes (e.g. bus) return empty until
			// at least one route is monitored, avoiding a useless all-routes request.
			if (source_info[source].monitor_routes && routes.length === 0) {
				return EMPTY_INDEX as StopTimeResource<S>;
			}

			const query_params = new URLSearchParams();
			const at = current_time.value;
			if (at) query_params.set('at', at.toString());
			// TODO: encodeURIComponent for route ids that contain special chars (e.g. "+")
			if (routes.length) query_params.set('route_ids', routes.join(','));

			const params_str = query_params.toString();
			const url = params_str
				? `/api/v1/stop_times/${source}?${params_str}`
				: `/api/v1/stop_times/${source}`;

			const res = await fetch(url, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error(`Failed to fetch stop times: ${res.status}`);

			const data: TypedStopTime<S>[] = await res.json();
			return index_stop_times<S>(data);
		},
		{
			initial_value,
			interval: source_info[source].refresh_interval.stop_times,
			debounce: 500
		}
	);

	let prev_time = current_time.value;
	$effect(() => {
		const val = current_time.value;
		if (val !== prev_time) {
			prev_time = val;
			resource.refresh();
		}
	});

	return {
		get value() {
			return resource.value;
		},
		get error() {
			return resource.error;
		},
		get is_fetching() {
			return resource.is_fetching;
		},
		get last_updated() {
			return resource.last_updated;
		},
		get offline() {
			return resource.offline;
		},

		get by_trip_id() {
			return resource.value?.by_trip_id ?? EMPTY_INDEX.by_trip_id;
		},
		get by_stop_id() {
			return resource.value?.by_stop_id ?? EMPTY_INDEX.by_stop_id;
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
				return resource.value ? Promise.resolve() : resource.next_refresh();
			}
			monitored_routes.add(route_id);
			return resource.next_refresh();
		},

		/**
		 * Stop monitoring a route. The change takes effect on the next refresh cycle;
		 * no immediate re-fetch is triggered.
		 */
		remove_route(route_id: string): void {
			monitored_routes.delete(route_id);
		},

		refresh(): Promise<void> | undefined {
			return resource.refresh();
		}
	};
}

export type StopTimeStore<S extends Source> = ReturnType<typeof createStopTimeResource<S>>;
export type StopTimeResources = { [S in Source]: StopTimeStore<S> };

export const stop_time_context = createMultiSourceContext<StopTimeResources>();
