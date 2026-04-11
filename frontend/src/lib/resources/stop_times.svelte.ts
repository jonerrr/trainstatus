import { SvelteMap } from 'svelte/reactivity';

import type { Source } from '$lib/client';
import {
	LiveResource,
	type StopTimeResource,
	type TypedStopTime,
	createMultiSourceContext,
	source_info
} from '$lib/resources/index.svelte';
import { current_time } from '$lib/url_params.svelte';

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

/**
 * Live stop times for a source: same `LiveResource` surface as trips/positions/alerts
 * (`current`, `status`, `refresh`, …) plus route monitoring helpers for sources that
 * require `route_ids` on the API.
 */
export class StopTimeLiveResource<S extends Source> extends LiveResource<StopTimeResource<S>> {
	#monitored_routes = new Set<string>();
	readonly #source: S;

	constructor(source: S) {
		const empty = EMPTY_INDEX as StopTimeResource<S>;
		super(
			async (signal) => {
				console.log(`updating ${source} stop times`);

				const routes = [...this.#monitored_routes];

				if (source_info[source].monitor_routes && routes.length === 0) {
					return empty;
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
			empty,
			{
				interval: source_info[source].refresh_interval.stop_times,
				debounce: 500
			}
		);
		this.#source = source;

		let prev_time = current_time.value;
		$effect(() => {
			const val = current_time.value;
			if (val !== prev_time) {
				prev_time = val;
				this.refresh();
			}
		});
	}

	get monitored_routes(): ReadonlySet<string> {
		return this.#monitored_routes;
	}

	add_route(route_id: string): Promise<void> {
		if (this.#monitored_routes.has(route_id)) {
			return this.status === 'ready' ? Promise.resolve() : this.next_refresh();
		}
		this.#monitored_routes.add(route_id);
		return this.next_refresh();
	}

	remove_route(route_id: string): void {
		this.#monitored_routes.delete(route_id);
	}

	/**
	 * Ensures routes are monitored (for monitor_routes sources) or data is
	 * loaded, then returns stop times for the given stop. The returned array
	 * is a reactive read from the underlying SvelteMap.
	 */
	async getForStop(stop_id: string, route_ids?: string[]): Promise<TypedStopTime<S>[]> {
		if (source_info[this.#source].monitor_routes && route_ids?.length) {
			await Promise.all(route_ids.map((id) => this.add_route(id)));
		} else {
			await this.whenReady();
		}
		return this.current?.by_stop_id.get(stop_id) ?? [];
	}

	/**
	 * Waits for data to be loaded, then returns stop times for the given trip.
	 * The returned array is a reactive read from the underlying SvelteMap.
	 */
	async getForTrip(trip_id: string): Promise<TypedStopTime<S>[]> {
		await this.whenReady();
		return this.current?.by_trip_id.get(trip_id) ?? [];
	}
}

export function createStopTimeResource<S extends Source>(source: S): StopTimeLiveResource<S> {
	return new StopTimeLiveResource(source);
}

export type StopTimeResources = Partial<{
	[S in Source]: StopTimeLiveResource<S>;
}>;

export const stop_time_context = createMultiSourceContext<StopTimeResources>();
