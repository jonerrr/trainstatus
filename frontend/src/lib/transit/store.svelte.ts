/* eslint-disable svelte/prefer-svelte-reactivity -- maps hold private entry identities;
   native state on each entry holds the reactive snapshots. */
import { createContext, untrack } from 'svelte';

import { on } from 'svelte/events';

import type {
	ApiAlert,
	Catalog,
	Service,
	ServiceTrip,
	Source,
	UnknownRoutes
} from '#lib/client/index.js';

import { decodeAlerts, type AlertsData } from './alerts.js';
import {
	callsAtStop,
	decodeCatalog,
	decodeService,
	emptyRoute,
	STALE_AFTER_MS,
	type Call,
	type CatalogData,
	type RouteData
} from './decode.js';

/** Sources whose service is always loaded and refreshed as one whole-source entry. */
const WHOLE_SOURCES: ReadonlySet<Source> = new Set(['mta_subway']);
const MAX_BATCH_ROUTES = 40;
const FIRST_RETRY_MS = 5_000;
const TICK_MS = 1_000;

export interface TransitDataOptions {
	sources: readonly Source[];
	at?: number;
	/** Map-only layouts defer passenger service until a binding acquires it. */
	eager_service?: boolean;
	fetcher?: typeof fetch;
	/** Live refresh interval while visible and online. */
	refresh_ms?: number;
	/** Historical entries retained beyond those actively read. */
	history_limit?: number;
	/** Quiet window for new bus routes to share a request (defaults to 200 ms). */
	batch_ms?: number;
	now?: () => number;
}

export interface TripRead {
	trip: ServiceTrip;
	/** Calls in visit order. */
	calls: readonly Call[];
	route: RouteData;
}

export interface StopArrivals {
	/** Calls at the stop from every loaded serving route, in arrival order. */
	calls: Call[];
	routes: ReadonlyMap<string, RouteData>;
	/** Serving routes whose first load has not finished. */
	pending: string[];
}

/** Separately polled data that reports into the shared status. */
export interface TrackedUpdate {
	/** Historical queries report failures but do not age into a live warning. */
	readonly at?: number | null;
	readonly active: boolean;
	readonly error: Error | null;
	readonly fetched_at: number | null;
	retry(): void;
}

export class UnknownRouteError extends Error {}

export interface ResourceSnapshot<T> {
	value: T | undefined;
	loading: boolean;
	/** Initial-load failure. Refresh failures retain a loaded value. */
	error: Error | null;
}

export interface Resource<T> {
	readonly current: ResourceSnapshot<T>;
}

type Kind = 'catalog' | 'alerts' | 'service';

class Entry<T> {
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
		readonly at: number | null,
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

type ServiceEntry = Entry<ReadonlyMap<string, RouteData>>;

class DataStatus {
	warning = $state(false);
}

interface Selection<T> {
	entries: Entry<unknown>[];
	project(): ResourceSnapshot<T>;
}

export interface RouteKey {
	source: Source;
	route_id: string;
	at?: number | null;
}

export interface CatalogRead {
	source: Source;
	catalog: CatalogData | undefined;
	loading: boolean;
	error: Error | null;
}

export interface RouteRead extends RouteKey {
	route: RouteData | undefined;
	loading: boolean;
	error: Error | null;
}

function serviceUrl(source: Source, routes: readonly string[] | null, at: number | null) {
	const params = new URLSearchParams();
	if (routes) params.set('route_ids', routes.join(','));
	if (at !== null) params.set('at', String(at));
	return `/api/v1/service/${source}${params.size ? `?${params}` : ''}`;
}

/**
 * Layout-scoped owner of catalogs, alerts and passenger service. Views use
 * native snapshots through bindings whose explicit effects own reader interest and first loads.
 */
export class TransitData {
	readonly status = new DataStatus();
	#fetcher: typeof fetch;
	#refresh_ms: number;
	#history_limit: number;
	#batch_ms: number;
	#eager_service: boolean;
	#at: number | null;
	#now: () => number;
	#sources = new Set<Source>();
	#generation = $state(0);
	#catalogs = new Map<Source, Entry<CatalogData>>();
	#alerts = new Map<string, Entry<AlertsData>>();
	#tracked = new Set<TrackedUpdate>();
	#services = new Map<string, ServiceEntry>();
	#queue = new Map<string, Set<ServiceEntry>>();
	#flush_timers = new Map<string, ReturnType<typeof setTimeout>>();
	#controllers = new Map<Source, AbortController>();
	#destroyed = false;
	#cleanup: (() => void)[] = [];

	constructor(options: TransitDataOptions) {
		this.#fetcher = options.fetcher ?? ((...args) => fetch(...args));
		this.#refresh_ms = options.refresh_ms ?? 30_000;
		this.#history_limit = options.history_limit ?? 6;
		this.#batch_ms = options.batch_ms ?? 200;
		this.#eager_service = options.eager_service ?? true;
		this.#at = options.at ?? null;
		this.#now = options.now ?? Date.now;
		untrack(() => this.setSources(options.sources));
		if (typeof window === 'undefined') return;
		const tick = () => this.#tick();
		const interval = setInterval(tick, TICK_MS);
		this.#cleanup.push(
			() => clearInterval(interval),
			on(document, 'visibilitychange', tick),
			on(window, 'online', tick),
			on(window, 'offline', tick)
		);
	}

	/** Enable sources, eagerly loading whole-source service; release disabled sources. */
	setSources(sources: readonly Source[]) {
		const next = new Set(sources);
		for (const source of this.#sources) if (!next.has(source)) this.#release(source);
		for (const source of next) {
			if (typeof window === 'undefined') break;
			if (this.#sources.has(source)) continue;
			const catalog = new Entry<CatalogData>(source, null, null, 'catalog');
			catalog.eager = true;
			this.#catalogs.set(source, catalog);
			void this.#loadCatalog(catalog);
			const alerts = this.#alertEntry(source, this.#at);
			alerts.eager = true;
			void this.#loadAlerts(alerts);
			if (!this.#eager_service || !WHOLE_SOURCES.has(source)) continue;
			const entry = this.#service(source, null, this.#at);
			entry.eager = true;
			this.#enqueue(entry);
		}
		if (next.size !== this.#sources.size || [...next].some((source) => !this.#sources.has(source)))
			this.#generation++;
		this.#sources = next;
	}

	/** Enable whole-source background service outside the map, keeping acquired readers active. */
	setEagerService(enabled: boolean) {
		if (enabled === this.#eager_service) return;
		this.#eager_service = enabled;
		for (const source of this.#sources) {
			if (!WHOLE_SOURCES.has(source)) continue;
			const entry = this.#service(source, null, this.#at);
			entry.eager = enabled;
			if (enabled) this.#ensure(entry);
		}
		this.#updateStatus();
	}

	/** Move background readers to the same time as the layout's passenger readers. */
	setTime(at: number | undefined) {
		const next = at ?? null;
		if (next === this.#at) return;
		this.#at = next;
		for (const entry of [...this.#alerts.values(), ...this.#services.values()]) entry.eager = false;
		for (const source of this.#sources) {
			const alerts = this.#alertEntry(source, next);
			alerts.eager = true;
			this.#ensure(alerts);
			if (!this.#eager_service || !WHOLE_SOURCES.has(source)) continue;
			const service = this.#service(source, null, next);
			service.eager = true;
			this.#ensure(service);
		}
		this.#updateStatus();
	}

	/** Bindings are created during component initialization, never inside a derived read. */
	#bind<T>(select: () => Selection<T>): Resource<T> {
		const selection = $derived.by(() => {
			void this.#generation;
			return select();
		});
		$effect(() => {
			const entries = selection.entries;
			return untrack(() => {
				for (const entry of entries) {
					entry.readers++;
					entry.used_at = this.#now();
					this.#ensure(entry);
				}
				this.#updateStatus();
				return () => {
					for (const entry of entries) entry.readers--;
				};
			});
		});
		return {
			get current() {
				return selection.project();
			}
		};
	}

	#catalogEntry(source: Source) {
		let entry = this.#catalogs.get(source);
		if (!entry) this.#catalogs.set(source, (entry = new Entry(source, null, null, 'catalog')));
		return entry;
	}

	catalog(source: () => Source | null): Resource<CatalogData> {
		return this.#bind(() => {
			const key = source();
			const entry = key === null ? undefined : this.#catalogEntry(key);
			return {
				entries: entry ? [entry] : [],
				project: () => entry?.current ?? { value: undefined, loading: false, error: null }
			};
		});
	}

	catalogs(sources: () => readonly Source[]): Resource<CatalogRead[]> {
		return this.#bind(() => {
			const entries = sources().map((source) => this.#catalogEntry(source));
			return {
				entries,
				project: () => ({
					value: entries.map((entry) => ({
						source: entry.source,
						catalog: entry.data,
						loading: entry.current.loading,
						error: entry.current.error
					})),
					loading: false,
					error: null
				})
			};
		});
	}

	alerts(key: () => { source: Source; at?: number | null } | null): Resource<AlertsData> {
		return this.#bind(() => {
			const selected = key();
			const entry = selected && this.#alertEntry(selected.source, selected.at ?? null);
			return {
				entries: entry ? [entry] : [],
				project: () => entry?.current ?? { value: undefined, loading: false, error: null }
			};
		});
	}

	#routeSnapshot(entry: ServiceEntry, route_id: string): ResourceSnapshot<RouteData> {
		const current = entry.current;
		return {
			...current,
			value:
				current.value &&
				(current.value.get(route_id) ??
					emptyRoute(entry.source, route_id, entry.at, entry.fetched_at))
		};
	}

	route(key: () => RouteKey | null): Resource<RouteData> {
		return this.#bind(() => {
			const selected = key();
			const entry =
				selected &&
				this.#service(
					selected.source,
					this.#key_route(selected.source, selected.route_id),
					selected.at ?? null
				);
			return {
				entries: entry ? [entry] : [],
				project: () =>
					selected && entry
						? this.#routeSnapshot(entry, selected.route_id)
						: { value: undefined, loading: false, error: null }
			};
		});
	}

	routes(keys: () => readonly RouteKey[]): Resource<RouteRead[]> {
		return this.#bind(() => {
			const selected = keys().map((key) => ({
				key,
				entry: this.#service(key.source, this.#key_route(key.source, key.route_id), key.at ?? null)
			}));
			return {
				entries: [...new Set(selected.map(({ entry }) => entry))],
				project: () => ({
					value: selected.map(({ key, entry }) => {
						const { value, loading, error } = this.#routeSnapshot(entry, key.route_id);
						return { ...key, route: value, loading, error };
					}),
					loading: false,
					error: null
				})
			};
		});
	}

	trip(key: () => (RouteKey & { trip_id: string }) | null): Resource<TripRead> {
		const route = this.route(key);
		const project = (current: ResourceSnapshot<RouteData>): ResourceSnapshot<TripRead> => {
			const selected = key();
			const data = current.value;
			const trip = selected && data?.trips.get(selected.trip_id);
			return {
				...current,
				value:
					selected && data && trip
						? { trip, calls: data.calls_by_trip.get(selected.trip_id) ?? [], route: data }
						: undefined
			};
		};
		return {
			get current() {
				return project(route.current);
			}
		};
	}

	stopArrivals(
		key: () => { source: Source; stop_id: string; at?: number | null }
	): Resource<StopArrivals> {
		return this.#bind(() => {
			const { source, stop_id, at = null } = key();
			const catalog = this.#catalogEntry(source);
			const ids = [
				...new Set(catalog.data?.stops_by_id.get(stop_id)?.routes.map((route) => route.route_id))
			];
			const selected = ids.map((id) => ({
				id,
				entry: this.#service(source, this.#key_route(source, id), at)
			}));
			return {
				entries: [catalog, ...new Set(selected.map(({ entry }) => entry))],
				project: () => {
					if (!catalog.data) return { ...catalog.current, value: undefined };
					const routes = new Map<string, RouteData>();
					const pending: string[] = [];
					let error: Error | null = null;
					for (const { id, entry } of selected) {
						const current = this.#routeSnapshot(entry, id);
						if (current.value) routes.set(id, current.value);
						else if (current.loading) pending.push(id);
						else error ??= current.error;
					}
					return {
						value: { calls: callsAtStop(routes.values(), stop_id), routes, pending },
						loading: !routes.size && pending.length > 0,
						error: !routes.size && !pending.length ? error : null
					};
				}
			};
		});
	}

	/** Pure peeks: observe available snapshots without acquiring service interest. */
	peekCatalog(source: Source): CatalogData | undefined {
		void this.#generation;
		return this.#catalogEntry(source).data;
	}

	loadedStopArrivals(
		source: Source,
		stop_id: string,
		at: number | null = null
	): Pick<StopArrivals, 'calls' | 'routes'> {
		const catalog = this.peekCatalog(source);
		const routes = new Map<string, RouteData>();
		for (const { route_id } of catalog?.stops_by_id.get(stop_id)?.routes ?? []) {
			const route = this.#peek(source, route_id, at);
			if (route) routes.set(route_id, route);
		}
		return { calls: callsAtStop(routes.values(), stop_id), routes };
	}

	loadedTrip(
		source: Source,
		route_id: string,
		trip_id: string,
		at: number | null = null
	): TripRead | undefined {
		const route = this.#peek(source, route_id, at);
		const trip = route?.trips.get(trip_id);
		return route && trip && { trip, calls: route.calls_by_trip.get(trip_id) ?? [], route };
	}

	/** Report separately polled trajectories without taking ownership of their requests. */
	track(update: TrackedUpdate): () => void {
		this.#tracked.add(update);
		return () => {
			this.#tracked.delete(update);
			this.#updateStatus();
		};
	}

	replaceCatalog(body: Catalog) {
		if (!this.#sources.has(body.source)) return;
		const catalog = decodeCatalog(body);
		this.#catalogEntry(body.source).install(catalog, this.#now());
		for (const entry of this.#services.values())
			if (
				entry.source === body.source &&
				entry.active &&
				entry.error instanceof UnknownRouteError &&
				entry.route_id !== null &&
				catalog.routes_by_id.has(entry.route_id)
			)
				this.#start(entry);
		this.#updateStatus();
	}

	#ensure(entry: Entry<unknown>) {
		if (this.#destroyed || entry.in_flight) return;
		if (entry.data !== undefined && (entry.kind === 'catalog' || entry.at !== null)) return;
		if (
			(entry.data !== undefined || entry.error) &&
			(this.#due(entry) > this.#now() || !this.#may_poll())
		)
			return;
		this.#start(entry);
	}

	#start(entry: Entry<unknown>) {
		if (this.#destroyed || entry.in_flight) return;
		entry.reset();
		if (entry.kind === 'catalog') void this.#loadCatalog(entry as Entry<CatalogData>);
		else if (entry.kind === 'alerts') void this.#loadAlerts(entry as Entry<AlertsData>);
		else this.#enqueue(entry as ServiceEntry);
	}

	/** Retry relevant initial failures and refresh live data, including aged successful reads. */
	retry() {
		for (const entry of [
			...this.#catalogs.values(),
			...this.#alerts.values(),
			...this.#services.values()
		]) {
			if (!entry.active && !entry.eager) continue;
			if (entry.kind === 'catalog' ? !entry.error : entry.at !== null && !entry.error) continue;
			this.#start(entry);
		}
		for (const update of this.#tracked) if (update.active) update.retry();
	}

	/** Stops timers and requests without touching reactive state that unmounting views may read. */
	destroy() {
		this.#destroyed = true;
		for (const cleanup of this.#cleanup) cleanup();
		for (const controller of this.#controllers.values()) controller.abort();
		this.#queue.clear();
		for (const timer of this.#flush_timers.values()) clearTimeout(timer);
		this.#flush_timers.clear();
	}

	#key_route(source: Source, route_id: string) {
		return WHOLE_SOURCES.has(source) ? null : route_id;
	}

	#service(source: Source, route_id: string | null, at: number | null): ServiceEntry {
		const key = `${source}|${at ?? ''}|${route_id ?? '*'}`;
		let entry = this.#services.get(key);
		if (!entry) this.#services.set(key, (entry = new Entry(source, at, route_id)));
		return entry;
	}

	#peek(source: Source, route_id: string, at: number | null): RouteData | undefined {
		void this.#generation;
		const entry = this.#service(source, this.#key_route(source, route_id), at);
		if (!entry.data) return undefined;
		return entry.data.get(route_id) ?? emptyRoute(source, route_id, at, entry.fetched_at);
	}

	#alertEntry(source: Source, at: number | null) {
		const key = `${source}|${at ?? ''}`;
		let entry = this.#alerts.get(key);
		if (!entry) this.#alerts.set(key, (entry = new Entry(source, at, null, 'alerts')));
		return entry;
	}

	#due(entry: Pick<ServiceEntry, 'error' | 'attempted_at' | 'failures' | 'fetched_at'>) {
		if (entry.error && entry.attempted_at !== null) {
			const backoff = FIRST_RETRY_MS * 2 ** Math.max(0, entry.failures - 1);
			return entry.attempted_at + Math.min(this.#refresh_ms, backoff);
		}
		return (entry.fetched_at ?? 0) + this.#refresh_ms;
	}

	#may_poll() {
		if (typeof document === 'undefined') return false;
		return document.visibilityState !== 'hidden' && navigator.onLine;
	}

	#controller(source: Source) {
		let controller = this.#controllers.get(source);
		if (!controller) this.#controllers.set(source, (controller = new AbortController()));
		return controller;
	}

	#loadCatalog(entry: Entry<CatalogData>) {
		return this.#load(entry, `/api/v1/catalog/${entry.source}`, {}, (body: Catalog) =>
			decodeCatalog(body)
		);
	}

	#loadAlerts(entry: Entry<AlertsData>) {
		const query = entry.at === null ? '' : `?at=${entry.at}`;
		return this.#load(
			entry,
			`/api/v1/alerts/${entry.source}${query}`,
			{ cache: 'no-store' },
			(body: ApiAlert[]) => decodeAlerts(body)
		);
	}

	async #load<B, T>(entry: Entry<T>, url: string, init: RequestInit, decode: (body: B) => T) {
		// Server renders show pending UI; the per-request owner never fetches.
		if (typeof window === 'undefined' || this.#destroyed) return;
		const { signal } = this.#controller(entry.source);
		entry.attempted_at = this.#now();
		entry.in_flight = true;
		const revision = entry.revision;
		try {
			const response = await this.#fetcher(url, {
				...init,
				signal: AbortSignal.any([signal, AbortSignal.timeout(15_000)])
			});
			if (!response.ok) throw new Error(`${url} returned ${response.status}`);
			const body = (await response.json()) as B;
			if (!signal.aborted && this.#owns(entry) && entry.revision === revision)
				entry.install(decode(body), this.#now());
		} catch (error) {
			if (!signal.aborted && this.#owns(entry) && entry.revision === revision)
				entry.fail(error as Error);
		} finally {
			entry.in_flight = false;
			this.#evictHistory();
			this.#updateStatus();
		}
	}

	#enqueue(entry: ServiceEntry) {
		if (typeof window === 'undefined' || this.#destroyed) return;
		const group = `${entry.source}|${entry.at ?? ''}`;
		let entries = this.#queue.get(group);
		if (!entries) this.#queue.set(group, (entries = new Set()));
		// Duplicate readers and polling ticks must not postpone an existing batch.
		if (entries.has(entry)) return;
		entries.add(entry);
		clearTimeout(this.#flush_timers.get(group));
		// Whole-source subway requests do not wait for scrolling bus rows.
		const delay = entry.route_id === null ? Math.min(10, this.#batch_ms) : this.#batch_ms;
		this.#flush_timers.set(
			group,
			setTimeout(() => this.#flush(group), delay)
		);
	}

	#flush(group: string) {
		this.#flush_timers.delete(group);
		const queued = this.#queue.get(group);
		this.#queue.delete(group);
		if (!queued) return;
		const entries = [...queued].filter(
			(entry) =>
				(entry.active || entry.eager) &&
				!entry.in_flight &&
				this.#services.get(this.#keyOf(entry)) === entry
		);
		if (!entries.length) return;
		if (entries[0].route_id === null) {
			void this.#request(entries.slice(0, 1));
			return;
		}
		entries.sort((a, b) => a.route_id!.localeCompare(b.route_id!));
		for (let i = 0; i < entries.length; i += MAX_BATCH_ROUTES) {
			void this.#request(entries.slice(i, i + MAX_BATCH_ROUTES));
		}
	}

	#keyOf(entry: ServiceEntry) {
		return `${entry.source}|${entry.at ?? ''}|${entry.route_id ?? '*'}`;
	}

	async #request(entries: ServiceEntry[]) {
		const { source, at } = entries[0];
		const routes = entries[0].route_id === null ? null : entries.map((entry) => entry.route_id!);
		const { signal } = this.#controller(source);
		const attempted = this.#now();
		for (const entry of entries) {
			entry.in_flight = true;
			entry.attempted_at = attempted;
		}
		try {
			const response = await this.#fetcher(serviceUrl(source, routes, at), {
				signal: AbortSignal.any([signal, AbortSignal.timeout(15_000)]),
				cache: 'no-store'
			});
			if (signal.aborted || this.#destroyed) return;
			if (response.status === 404 && routes) {
				const unknown = new Set(((await response.json()) as UnknownRoutes).unknown_route_ids);
				if (signal.aborted || this.#destroyed) return;
				for (const entry of entries) {
					if (!this.#owns(entry)) continue;
					entry.in_flight = false;
					if (!unknown.has(entry.route_id!)) this.#enqueue(entry);
					else {
						entry.fail(new UnknownRouteError(`Unknown ${source} route ${entry.route_id}`));
					}
				}
				return;
			}
			if (!response.ok) throw new Error(`Service returned ${response.status}`);
			const service = (await response.json()) as Service;
			const now = this.#now();
			const decoded = decodeService(service, now);
			for (const entry of entries) {
				if (signal.aborted || !this.#owns(entry)) continue;
				entry.in_flight = false;
				entry.source_updated_at =
					at === null && service.updated_at != null ? service.updated_at * 1000 : null;
				if (routes === null) entry.install(decoded, now);
				else {
					const route = decoded.get(entry.route_id!);
					if (route) entry.install(new Map([[entry.route_id!, route]]), now);
					else entry.fail(new Error(`Service omitted route ${entry.route_id}`));
				}
			}
		} catch (error) {
			if (signal.aborted) return;
			for (const entry of entries) {
				if (signal.aborted || !this.#owns(entry)) continue;
				entry.in_flight = false;
				entry.fail(error as Error);
			}
		} finally {
			this.#evictHistory();
			this.#updateStatus();
		}
	}

	#evictHistory() {
		type Evictable = Pick<ServiceEntry, 'at' | 'active' | 'eager' | 'in_flight' | 'used_at'>;
		for (const entries of [this.#services, this.#alerts] as Map<string, Evictable>[]) {
			const evictable = [...entries.entries()]
				.filter(
					([, entry]) => entry.at !== null && !entry.active && !entry.eager && !entry.in_flight
				)
				.sort(([, a], [, b]) => a.used_at - b.used_at);
			const historical = [...entries.values()].filter((entry) => entry.at !== null).length;
			for (const [key] of evictable.slice(0, Math.max(0, historical - this.#history_limit))) {
				entries.delete(key);
			}
		}
	}

	#tick() {
		this.#updateStatus();
		if (!this.#may_poll()) return;
		for (const entry of [
			...this.#catalogs.values(),
			...this.#services.values(),
			...this.#alerts.values()
		]) {
			if (!entry.active && !entry.eager) continue;
			if (entry.kind === 'catalog' ? !entry.error : entry.at !== null) continue;
			this.#ensure(entry);
		}
		this.#evictHistory();
	}

	#updateStatus() {
		if (this.#destroyed) return;
		let warning = typeof navigator !== 'undefined' && !navigator.onLine;
		const now = this.#now();
		for (const entry of [
			...this.#catalogs.values(),
			...this.#services.values(),
			...this.#alerts.values()
		]) {
			if (entry.error instanceof UnknownRouteError || (!entry.active && !entry.eager)) continue;
			if (entry.error) warning = true;
			if (entry.kind === 'catalog' || entry.at !== null || entry.fetched_at === null) continue;
			const refreshed = Math.min(entry.fetched_at, entry.source_updated_at ?? entry.fetched_at);
			if (now - refreshed > STALE_AFTER_MS) warning = true;
		}
		for (const update of this.#tracked) {
			if (!update.active) continue;
			if (
				update.error ||
				(update.at == null &&
					update.fetched_at !== null &&
					now - update.fetched_at > STALE_AFTER_MS)
			)
				warning = true;
		}
		this.status.warning = warning;
	}

	#owns(entry: Entry<unknown>) {
		if (this.#destroyed) return false;
		if (entry.kind === 'catalog') return this.#catalogs.get(entry.source) === entry;
		if (entry.kind === 'alerts')
			return this.#alerts.get(`${entry.source}|${entry.at ?? ''}`) === entry;
		return this.#services.get(this.#keyOf(entry as ServiceEntry)) === entry;
	}

	#release(source: Source) {
		this.#controllers.get(source)?.abort();
		this.#controllers.delete(source);
		this.#catalogs.delete(source);
		for (const [key, entry] of this.#alerts) if (entry.source === source) this.#alerts.delete(key);
		for (const [key, entry] of this.#services)
			if (entry.source === source) this.#services.delete(key);
		for (const [group, entries] of this.#queue) {
			if (![...entries].some((entry) => entry.source === source)) continue;
			this.#queue.delete(group);
			clearTimeout(this.#flush_timers.get(group));
			this.#flush_timers.delete(group);
		}
	}
}

export const [getTransitData, setTransitData] = createContext<TransitData>();
