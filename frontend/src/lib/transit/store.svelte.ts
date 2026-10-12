/* eslint-disable svelte/prefer-svelte-reactivity -- readers subscribe per entry through
   createSubscriber; collections here are private bookkeeping, never read reactively. */
import { createContext } from 'svelte';

import { on } from 'svelte/events';
import { createSubscriber } from 'svelte/reactivity';

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
	fetcher?: typeof fetch;
	/** Live refresh interval while visible and online. */
	refresh_ms?: number;
	/** Historical entries retained beyond those actively read. */
	history_limit?: number;
	/** Delay that lets independently requested bus routes share one request. */
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
	/** Serving routes whose first load failed. */
	failed: string[];
}

/** Live data polled outside this module that reports into the shared status. */
export interface TrackedUpdate {
	readonly active: boolean;
	readonly error: Error | null;
	readonly fetched_at: number | null;
	retry(): void;
}

export class UnknownRouteError extends Error {}

class Entry<T> {
	data: T | undefined = undefined;
	promise: Promise<T> | undefined = undefined;
	active = false;
	/** Live whole-source entries refresh without readers. */
	eager = false;
	unknown = false;
	in_flight = false;
	fetched_at: number | null = null;
	attempted_at: number | null = null;
	failures = 0;
	error: Error | null = null;
	used_at = 0;
	readonly subscribe: () => void;
	#update = () => {};
	#resolve: ((value: T) => void) | undefined;
	#reject: ((error: Error) => void) | undefined;

	constructor(
		readonly source: Source,
		readonly at: number | null,
		readonly route_id: string | null
	) {
		this.subscribe = createSubscriber((update) => {
			this.#update = update;
			this.active = true;
			return () => {
				this.active = false;
			};
		});
	}

	/** The shared first-load promise, created on demand. */
	pending(): Promise<T> {
		if (!this.promise) {
			this.promise = new Promise<T>((resolve, reject) => {
				this.#resolve = resolve;
				this.#reject = reject;
			});
			// Rejections surface through readers; an unread one is not an error.
			this.promise.catch(() => {});
		}
		return this.promise;
	}

	install(data: T, now: number) {
		this.data = data;
		this.fetched_at = now;
		this.failures = 0;
		this.error = null;
		this.#resolve?.(data);
		this.promise = this.#resolve = this.#reject = undefined;
		this.#update();
	}

	fail(error: Error) {
		this.error = error;
		this.failures++;
		if (this.data === undefined) this.#reject?.(error);
		this.#resolve = this.#reject = undefined;
		this.#update();
	}

	/** Forget a failed first load so the next read requests it again. */
	reset() {
		if (this.data === undefined && this.error) this.promise = undefined;
	}
}

type ServiceEntry = Entry<ReadonlyMap<string, RouteData>>;

class DataStatus {
	offline = $state(false);
	/** The latest attempt for some actively read live data failed. */
	failing = $state(false);
	/** Oldest last successful fetch among actively read live data, epoch milliseconds. */
	last_fetched = $state<number | null>(null);
	/** Some actively read live data is older than the stale threshold. */
	stale = $state(false);
	/** Increments on each explicit retry, so failed views can reset. */
	retries = $state(0);
}

/** A read's value when it is already available, without waiting. */
export function loaded<T>(read: T | Promise<T>): T | undefined {
	if (!(read instanceof Promise)) return read;
	// Failures surface through status and boundaries; this caller only peeks.
	read.catch(() => {});
	return undefined;
}

function serviceUrl(source: Source, routes: readonly string[] | null, at: number | null) {
	const params = new URLSearchParams();
	if (routes) params.set('route_ids', routes.join(','));
	if (at !== null) params.set('at', String(at));
	return `/api/v1/service/${source}${params.size ? `?${params}` : ''}`;
}

/**
 * Layout-scoped owner of catalogs and passenger service. Reads return loaded data
 * synchronously and otherwise a shared promise; readers subscribe while they render.
 */
export class TransitData {
	readonly status = new DataStatus();
	#fetcher: typeof fetch;
	#refresh_ms: number;
	#history_limit: number;
	#batch_ms: number;
	#now: () => number;
	#sources = new Set<Source>();
	#catalogs = new Map<Source, Entry<CatalogData>>();
	#alerts = new Map<string, Entry<AlertsData>>();
	#tracked = new Set<TrackedUpdate>();
	#services = new Map<string, ServiceEntry>();
	#queue = new Map<string, Set<ServiceEntry>>();
	#flush_timer: ReturnType<typeof setTimeout> | undefined;
	#controllers = new Map<Source, AbortController>();
	#destroyed = false;
	#cleanup: (() => void)[] = [];

	constructor(options: TransitDataOptions) {
		this.#fetcher = options.fetcher ?? ((...args) => fetch(...args));
		this.#refresh_ms = options.refresh_ms ?? 30_000;
		this.#history_limit = options.history_limit ?? 6;
		this.#batch_ms = options.batch_ms ?? 10;
		this.#now = options.now ?? Date.now;
		this.setSources(options.sources);
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
			const catalog = new Entry<CatalogData>(source, null, null);
			this.#catalogs.set(source, catalog);
			void this.#loadCatalog(catalog);
			const alerts = this.#alertEntry(source, null);
			alerts.eager = true;
			void this.#loadAlerts(alerts);
			if (!WHOLE_SOURCES.has(source)) continue;
			const entry = this.#service(source, null, null);
			entry.eager = true;
			this.#enqueue(entry);
		}
		this.#sources = next;
	}

	catalog(source: Source): CatalogData | Promise<CatalogData> {
		let entry = this.#catalogs.get(source);
		if (!entry) this.#catalogs.set(source, (entry = new Entry(source, null, null)));
		entry.subscribe();
		if (entry.data) return entry.data;
		if (!entry.promise) void this.#loadCatalog(entry);
		return entry.pending();
	}

	/** Non-blocking catalog read for views that render each source as it arrives. */
	catalogState(source: Source): { catalog: CatalogData | undefined; error: Error | null } {
		const catalog = loaded(this.catalog(source));
		return { catalog, error: catalog ? null : (this.#catalogs.get(source)?.error ?? null) };
	}

	/** Current alerts for a source, or those active at `at` (Unix seconds). */
	alerts(source: Source, at: number | null = null): AlertsData | Promise<AlertsData> {
		const entry = this.#alertEntry(source, at);
		entry.subscribe();
		entry.used_at = this.#now();
		if (entry.data) {
			if (at === null && !entry.in_flight && this.#due(entry) <= this.#now() && this.#may_poll())
				void this.#loadAlerts(entry);
			return entry.data;
		}
		if (!entry.promise) void this.#loadAlerts(entry);
		return entry.pending();
	}

	/** Report externally polled live data in the shared status; returns an unregister. */
	track(update: TrackedUpdate): () => void {
		this.#tracked.add(update);
		return () => {
			this.#tracked.delete(update);
			this.#updateStatus();
		};
	}

	/** Install a newer catalog body, e.g. from service-worker revalidation. */
	replaceCatalog(body: Catalog) {
		let entry = this.#catalogs.get(body.source);
		if (!entry) this.#catalogs.set(body.source, (entry = new Entry(body.source, null, null)));
		entry.install(decodeCatalog(body), this.#now());
	}

	/** Non-blocking route read for views that render each route as it arrives. */
	routeState(
		source: Source,
		route_id: string,
		at: number | null = null
	): { route: RouteData | undefined; error: Error | null } {
		const route = loaded(this.route(source, route_id, at));
		const entry = this.#service(source, this.#key_route(source, route_id), at);
		return { route, error: route ? null : entry.error };
	}

	/** Complete service for a route; `at` (Unix seconds) selects retained history. */
	route(
		source: Source,
		route_id: string,
		at: number | null = null
	): RouteData | Promise<RouteData> {
		const entry = this.#read(source, route_id, at);
		const pick = (routes: ReadonlyMap<string, RouteData>) =>
			routes.get(route_id) ?? emptyRoute(source, route_id, at, entry.fetched_at);
		if (entry.data) return pick(entry.data);
		return entry.pending().then(pick);
	}

	trip(
		source: Source,
		route_id: string,
		trip_id: string,
		at: number | null = null
	): TripRead | undefined | Promise<TripRead | undefined> {
		const pick = (route: RouteData): TripRead | undefined => {
			const trip = route.trips.get(trip_id);
			return trip && { trip, calls: route.calls_by_trip.get(trip_id) ?? [], route };
		};
		const route = this.route(source, route_id, at);
		return route instanceof Promise ? route.then(pick) : pick(route);
	}

	/**
	 * Arrivals at a stop across its serving routes. Waits only while no serving route
	 * has loaded; afterwards, routes that are still loading or failed are listed.
	 */
	stopArrivals(
		source: Source,
		stop_id: string,
		at: number | null = null
	): StopArrivals | Promise<StopArrivals> {
		const catalog = this.catalog(source);
		if (catalog instanceof Promise)
			return catalog.then(() => this.stopArrivals(source, stop_id, at));
		const route_ids = [
			...new Set(catalog.stops_by_id.get(stop_id)?.routes.map((route) => route.route_id))
		];
		const routes = new Map<string, RouteData>();
		const pending: string[] = [];
		const failed: string[] = [];
		const waits: Promise<unknown>[] = [];
		for (const route_id of route_ids) {
			const entry = this.#read(source, route_id, at);
			if (entry.data)
				routes.set(
					route_id,
					entry.data.get(route_id) ?? emptyRoute(source, route_id, at, entry.fetched_at)
				);
			else if (entry.error) failed.push(route_id);
			else {
				pending.push(route_id);
				waits.push(entry.pending());
			}
		}
		const result = () => ({
			calls: callsAtStop(routes.values(), stop_id),
			routes,
			pending,
			failed
		});
		if (routes.size || !route_ids.length) return result();
		if (!waits.length) return Promise.reject(this.#entryError(source, failed[0], at));
		return Promise.allSettled(waits).then(() => this.stopArrivals(source, stop_id, at));
	}

	/**
	 * Non-blocking stop arrivals for rows that render as data arrives. `read` is set once
	 * any serving route has loaded; `error` only when the first load failed outright.
	 */
	stopArrivalsState(
		source: Source,
		stop_id: string,
		at: number | null = null
	): { read: StopArrivals | undefined; error: Error | null } {
		const read = this.stopArrivals(source, stop_id, at);
		if (!(read instanceof Promise)) return { read, error: null };
		read.catch(() => {});
		const catalog = this.#catalogs.get(source);
		if (!catalog?.data) return { read: undefined, error: catalog?.error ?? null };
		const route_ids = new Set(
			catalog.data.stops_by_id.get(stop_id)?.routes.map((route) => route.route_id)
		);
		const errors = [...route_ids].map(
			(route_id) => this.#service(source, this.#key_route(source, route_id), at).error
		);
		return { read: undefined, error: errors.length && errors.every(Boolean) ? errors[0] : null };
	}

	/** Alerts for views that render each row as data arrives. */
	alertsState(
		source: Source,
		at: number | null = null
	): { alerts: AlertsData | undefined; error: Error | null } {
		const alerts = loaded(this.alerts(source, at));
		return { alerts, error: alerts ? null : this.#alertEntry(source, at).error };
	}

	/** Arrivals from serving routes that are already loaded; requests nothing. */
	loadedStopArrivals(
		source: Source,
		stop_id: string,
		at: number | null = null
	): Pick<StopArrivals, 'calls' | 'routes'> {
		const catalog = loaded(this.catalog(source));
		const routes = new Map<string, RouteData>();
		for (const { route_id } of catalog?.stops_by_id.get(stop_id)?.routes ?? []) {
			const route = this.#peek(source, route_id, at);
			if (route) routes.set(route_id, route);
		}
		return { calls: callsAtStop(routes.values(), stop_id), routes };
	}

	/** A trip from its route if already loaded; requests nothing. */
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

	/** Retry failed first loads and failed live refreshes now. */
	retry() {
		this.status.retries++;
		for (const entry of this.#catalogs.values()) {
			if (entry.data || !entry.error) continue;
			entry.reset();
			void this.#loadCatalog(entry);
		}
		for (const entry of this.#alerts.values()) {
			if (!entry.error || entry.in_flight) continue;
			entry.reset();
			void this.#loadAlerts(entry);
		}
		for (const update of this.#tracked) if (update.error) update.retry();
		for (const entry of this.#services.values()) {
			if (!entry.error || entry.unknown || entry.in_flight) continue;
			entry.reset();
			this.#enqueue(entry);
		}
	}

	/** Refetch actively read historical data, which otherwise loads once. */
	reload() {
		this.retry();
		for (const entry of this.#services.values()) {
			if (entry.at !== null && entry.active && !entry.in_flight && !entry.unknown)
				this.#enqueue(entry);
		}
		for (const entry of this.#alerts.values()) {
			if (entry.at !== null && entry.active && !entry.in_flight) void this.#loadAlerts(entry);
		}
		for (const update of this.#tracked) if (update.active) update.retry();
	}

	/** Stops timers and requests without touching reactive state that unmounting views may read. */
	destroy() {
		this.#destroyed = true;
		for (const cleanup of this.#cleanup) cleanup();
		for (const controller of this.#controllers.values()) controller.abort();
		this.#queue.clear();
		clearTimeout(this.#flush_timer);
	}

	#entryError(source: Source, route_id: string, at: number | null) {
		return this.#service(source, this.#key_route(source, route_id), at).error!;
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
		const entry = this.#service(source, this.#key_route(source, route_id), at);
		entry.subscribe();
		if (!entry.data) return undefined;
		return entry.data.get(route_id) ?? emptyRoute(source, route_id, at, entry.fetched_at);
	}

	#read(source: Source, route_id: string, at: number | null): ServiceEntry {
		const entry = this.#service(source, this.#key_route(source, route_id), at);
		entry.subscribe();
		entry.used_at = this.#now();
		if (entry.in_flight || entry.unknown) return entry;
		if (entry.data === undefined) {
			if (!entry.promise) this.#enqueue(entry);
		} else if (at === null && this.#due(entry) <= this.#now() && this.#may_poll()) {
			this.#enqueue(entry);
		}
		return entry;
	}

	#alertEntry(source: Source, at: number | null) {
		const key = `${source}|${at ?? ''}`;
		let entry = this.#alerts.get(key);
		if (!entry) this.#alerts.set(key, (entry = new Entry(source, at, null)));
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
		if (entry.data === undefined) entry.pending();
		// Server renders show pending UI; the per-request owner never fetches.
		if (typeof window === 'undefined' || this.#destroyed) return;
		const { signal } = this.#controller(entry.source);
		entry.attempted_at = this.#now();
		entry.in_flight = true;
		try {
			const response = await this.#fetcher(url, { ...init, signal });
			if (!response.ok) throw new Error(`${url} returned ${response.status}`);
			entry.install(decode((await response.json()) as B), this.#now());
		} catch (error) {
			if (!signal.aborted) entry.fail(error as Error);
		} finally {
			entry.in_flight = false;
			this.#updateStatus();
		}
	}

	#enqueue(entry: ServiceEntry) {
		if (entry.data === undefined) entry.pending();
		if (typeof window === 'undefined' || this.#destroyed) return;
		const group = `${entry.source}|${entry.at ?? ''}`;
		let entries = this.#queue.get(group);
		if (!entries) this.#queue.set(group, (entries = new Set()));
		entries.add(entry);
		this.#flush_timer ??= setTimeout(() => this.#flush(), this.#batch_ms);
	}

	#flush() {
		this.#flush_timer = undefined;
		const groups = [...this.#queue.values()];
		this.#queue.clear();
		for (const group of groups) {
			const entries = [...group].filter(
				(entry) => !entry.in_flight && this.#services.get(this.#keyOf(entry)) === entry
			);
			if (!entries.length) continue;
			if (entries[0].route_id === null) {
				void this.#request(entries.slice(0, 1));
				continue;
			}
			entries.sort((a, b) => a.route_id!.localeCompare(b.route_id!));
			for (let i = 0; i < entries.length; i += MAX_BATCH_ROUTES) {
				void this.#request(entries.slice(i, i + MAX_BATCH_ROUTES));
			}
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
				signal,
				cache: 'no-store'
			});
			if (response.status === 404 && routes) {
				const unknown = new Set(((await response.json()) as UnknownRoutes).unknown_route_ids);
				for (const entry of entries) {
					entry.in_flight = false;
					if (!unknown.has(entry.route_id!)) this.#enqueue(entry);
					else {
						entry.unknown = true;
						entry.fail(new UnknownRouteError(`Unknown ${source} route ${entry.route_id}`));
					}
				}
				return;
			}
			if (!response.ok) throw new Error(`Service returned ${response.status}`);
			const now = this.#now();
			const decoded = decodeService((await response.json()) as Service, now);
			for (const entry of entries) {
				entry.in_flight = false;
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
				entry.in_flight = false;
				entry.fail(error as Error);
			}
		} finally {
			this.#evictHistory();
			this.#updateStatus();
		}
	}

	#evictHistory() {
		type Evictable = Pick<ServiceEntry, 'at' | 'active' | 'in_flight' | 'used_at'>;
		for (const entries of [this.#services, this.#alerts] as Map<string, Evictable>[]) {
			const evictable = [...entries.entries()]
				.filter(([, entry]) => entry.at !== null && !entry.active && !entry.in_flight)
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
		const now = this.#now();
		for (const entry of this.#services.values()) {
			if (entry.at !== null || entry.unknown || entry.in_flight) continue;
			if (!entry.active && !entry.eager) continue;
			if (entry.data === undefined && !entry.error) continue;
			if (this.#due(entry) > now) continue;
			entry.reset();
			this.#enqueue(entry);
		}
		for (const entry of this.#alerts.values()) {
			if (entry.at !== null || entry.in_flight || (!entry.active && !entry.eager)) continue;
			if (this.#due(entry) > now) continue;
			entry.reset();
			void this.#loadAlerts(entry);
		}
		this.#evictHistory();
	}

	#updateStatus() {
		this.status.offline = typeof navigator !== 'undefined' && !navigator.onLine;
		let failing = false;
		let last_fetched: number | null = null;
		const live: Pick<TrackedUpdate, 'error' | 'fetched_at'>[] = [...this.#tracked].filter(
			(update) => update.active
		);
		for (const entry of [...this.#services.values(), ...this.#alerts.values()]) {
			if (entry.at !== null || entry.unknown || (!entry.active && !entry.eager)) continue;
			live.push(entry);
		}
		for (const entry of live) {
			if (entry.error) failing = true;
			if (entry.fetched_at !== null) {
				last_fetched =
					last_fetched === null ? entry.fetched_at : Math.min(last_fetched, entry.fetched_at);
			}
		}
		this.status.failing = failing;
		this.status.last_fetched = last_fetched;
		this.status.stale = last_fetched !== null && this.#now() - last_fetched > STALE_AFTER_MS;
	}

	#release(source: Source) {
		this.#controllers.get(source)?.abort();
		this.#controllers.delete(source);
		this.#catalogs.delete(source);
		for (const [key, entry] of this.#alerts) if (entry.source === source) this.#alerts.delete(key);
		for (const [key, entry] of this.#services)
			if (entry.source === source) this.#services.delete(key);
		for (const [group, entries] of this.#queue) {
			if ([...entries].some((entry) => entry.source === source)) this.#queue.delete(group);
		}
	}
}

export const [getTransitData, setTransitData] = createContext<TransitData>();
