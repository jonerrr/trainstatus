import { untrack } from 'svelte';

import { SvelteDate, SvelteMap, SvelteSet } from 'svelte/reactivity';

import type { Source } from '#lib/client/index.js';
import {
	createMultiSourceContext,
	LiveResource,
	source_info,
	type StopTimeResource,
	type TypedStopTime
} from '#lib/resources/index.svelte.js';
import { getCurrentTime } from '#lib/url_params.svelte.js';

import { requestData, resourceQuery, type ResourceQuery } from './request';
import { trip_context } from './trips.svelte';

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

function emptyIndex<S extends Source>(): StopTimeResource<S> {
	return { by_trip_id: new SvelteMap(), by_stop_id: new SvelteMap() };
}

/** Keep rows for routes that remain. Drop a row only when its trip names a released route. */
export function retain_stop_times<S extends Source>(
	data: StopTimeResource<S>,
	routeOf: (tripId: string) => string | undefined,
	next: Pick<ResourceQuery, 'active' | 'routes'>
): StopTimeResource<S> {
	if (!next.active) return emptyIndex();
	const retained = [...data.by_trip_id.values()].flat().filter((st) => {
		const route = routeOf(st.trip_id);
		return route === undefined || next.routes.includes(route);
	});
	return index_stop_times(retained);
}

export class StopTimeLiveResource<S extends Source> extends LiveResource<StopTimeResource<S>> {
	// Counts are bookkeeping, revision is the reactive query trigger.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#holders = new Map<string, number>();
	#revision = $state(0);
	constructor(source: S, fetcher: typeof fetch = fetch) {
		const time = getCurrentTime();
		const trips = trip_context.getSource(source);
		// Called after construction by the owning effect or route acquisition.
		const query = () => {
			void this.#revision;
			const routes = [...this.#holders.keys()];
			return resourceQuery(
				source,
				'stop_times',
				time.value ?? null,
				source_info[source].monitor_routes ? routes : [],
				!source_info[source].monitor_routes || routes.length > 0
			);
		};
		super(
			(captured, signal) =>
				requestData(
					captured.url,
					async (response) => index_stop_times<S>(await response.json()),
					signal,
					fetcher
				),
			emptyIndex<S>(),
			{
				query,
				interval: source_info[source].refresh_interval.stop_times,
				debounce: 500,
				retain: (data, _previous, next) =>
					retain_stop_times(data, (tripId) => trips?.current.get(tripId)?.route_id, next)
			}
		);
	}
	add_route(route: string): Promise<StopTimeResource<S>> {
		return untrack(() => {
			const count = this.#holders.get(route) ?? 0;
			this.#holders.set(route, count + 1);
			if (count === 0) this.#revision++;
			return this.whenAvailable(route);
		});
	}
	remove_route(route: string) {
		untrack(() => {
			const count = this.#holders.get(route);
			if (count === undefined) return;
			if (count > 1) {
				this.#holders.set(route, count - 1);
				return;
			}
			this.#holders.delete(route);
			this.#revision++;
			this.syncQuery();
		});
	}
	/** Reference-count these routes until the caller runs the returned cleanup. */
	hold(routes: readonly string[]) {
		for (const route of routes) void this.add_route(route).catch(() => {});
		return () => {
			for (const route of routes) this.remove_route(route);
		};
	}
	/**
	 * One caller's route set. Syncing replaces that set and leaves routes that
	 * remain held, so a visible row is not released and acquired again.
	 */
	sync_routes() {
		const held = new SvelteSet<string>();
		const sync = (routes: Iterable<string>) => {
			const next = new SvelteSet(routes);
			for (const route of next) {
				if (!held.has(route)) void this.add_route(route).catch(() => {});
			}
			for (const route of held) {
				if (!next.has(route)) this.remove_route(route);
			}
			held.clear();
			for (const route of next) held.add(route);
		};
		return {
			sync,
			stop: () => sync([])
		};
	}
}

export function createStopTimeResource<S extends Source>(source: S): StopTimeLiveResource<S> {
	return new StopTimeLiveResource(source);
}

export type StopTimeResources = Partial<{
	[S in Source]: StopTimeLiveResource<S>;
}>;

export const stop_time_context = createMultiSourceContext<StopTimeResources>();
