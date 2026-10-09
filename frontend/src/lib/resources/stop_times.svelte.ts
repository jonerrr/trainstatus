import { SvelteDate, SvelteMap, SvelteURLSearchParams } from 'svelte/reactivity';

import type { Source } from '#lib/client/index.js';
import {
	createMultiSourceContext,
	LiveResource,
	source_info,
	type StopTimeResource,
	type TypedStopTime
} from '#lib/resources/index.svelte.js';
import { getCurrentTime } from '#lib/url_params.svelte.js';

export function index_stop_times<S extends Source>(data: TypedStopTime<S>[]): StopTimeResource<S> {
	const by_trip_id = new SvelteMap<string, TypedStopTime<S>[]>();
	const by_stop_id = new SvelteMap<string, TypedStopTime<S>[]>();

	for (const st of data) {
		const typed_st = {
			...st,
			arrival: new SvelteDate(st.arrival),
			departure: new SvelteDate(st.departure)
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

/**
 * Live stop times for a source: same `LiveResource` surface as trips/positions/alerts
 * (`current`, `status`, `refresh`, …) plus route monitoring helpers for sources that
 * require `route_ids` on the API.
 */
export class StopTimeLiveResource<S extends Source> extends LiveResource<StopTimeResource<S>> {
	/** route_id → number of active holders */
	// Reference counts are fetch bookkeeping, not UI state; keep them untracked.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#monitored_routes = new Map<string, number>();

	constructor(source: S) {
		const current_time = getCurrentTime();
		const empty = EMPTY_INDEX as StopTimeResource<S>;
		super(
			async (signal) => {
				console.log(`updating ${source} stop times`);

				const routes = [...this.#monitored_routes.keys()];

				if (source_info[source].monitor_routes && routes.length === 0) {
					return empty;
				}

				const query_params = new SvelteURLSearchParams();
				const at = current_time.value;
				if (at !== undefined) query_params.set('at', at.toString());
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
			empty,
			{
				interval: source_info[source].refresh_interval.stop_times,
				debounce: 500
			}
		);

		let prev_time = current_time.value;
		$effect(() => {
			const val = current_time.value;
			if (val !== prev_time) {
				prev_time = val;
				this.refresh();
			}
		});
	}

	add_route(route_id: string): Promise<void> {
		const count = this.#monitored_routes.get(route_id) ?? 0;
		this.#monitored_routes.set(route_id, count + 1);
		// Only trigger a fetch the first time this route is registered
		if (count === 0) {
			return this.next_refresh();
		}
		return this.status === 'ready' ? Promise.resolve() : this.next_refresh();
	}

	remove_route(route_id: string): void {
		const count = this.#monitored_routes.get(route_id);
		if (count === undefined) return;
		if (count <= 1) {
			this.#monitored_routes.delete(route_id);
		} else {
			this.#monitored_routes.set(route_id, count - 1);
		}
	}
}

export function createStopTimeResource<S extends Source>(source: S): StopTimeLiveResource<S> {
	return new StopTimeLiveResource(source);
}

export type StopTimeResources = Partial<{
	[S in Source]: StopTimeLiveResource<S>;
}>;

export const stop_time_context = createMultiSourceContext<StopTimeResources>();
