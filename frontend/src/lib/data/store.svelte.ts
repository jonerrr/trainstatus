/* eslint-disable svelte/prefer-svelte-reactivity -- maps hold private entry identities;
   native state on each entry holds the reactive snapshots. */
import { createContext, untrack } from 'svelte';

import { on } from 'svelte/events';

import { browser } from '$app/env';

import type {
	ApiAlert,
	Catalog,
	Service,
	ServiceTrip,
	Source,
	UnknownRoutes
} from '#lib/client/index.js';
import { sourceConfig } from '#lib/sources.js';

import { decodeAlerts, type AlertsData } from './alerts.js';
import {
	callsAtStop,
	decodeCatalog,
	decodeService,
	STALE_AFTER_MS,
	type Call,
	type CatalogData,
	type RouteData
} from './decode.js';
import { Entry, toError, type Read, type Resource } from './resource.svelte.js';

export type { Resource, Read } from './resource.svelte.js';

const MAX_BATCH_ROUTES = 40;
const FIRST_RETRY_MS = 5_000;
const TICK_MS = 1_000;

export interface TransitDataOptions {
	sources: readonly Source[];
	at?: number;
	/** Map-only layouts defer passenger service until a binding acquires it. */
	eagerService?: boolean;
	fetcher?: typeof fetch;
	/** Live refresh interval while visible and online. */
	refreshMs?: number;
	/** Historical entries retained beyond those actively read. */
	historyLimit?: number;
	/** Quiet window for new bus routes to share a request (defaults to 200 ms). */
	batchMs?: number;
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
	readonly at?: number;
	readonly active: boolean;
	readonly error: Error | null;
	/** Decoding/programming failures use the shared error screen instead of the Retry warning. */
	readonly fatal?: boolean;
	readonly fetchedAt: number | null;
	readonly sourceUpdatedAt?: number | null;
	retry(): void;
}

type ServiceEntry = Entry<ReadonlyMap<string, RouteData>>;

// One binding may combine several request entries (for example, all routes at a stop).
interface Selection<T> {
	entries: Entry<unknown>[];
	read(): T;
}

export interface RouteKey {
	source: Source;
	routeId: string;
	at?: number;
}

export interface TripKey extends RouteKey {
	tripId: string;
}

interface TripSelection {
	requested: TripKey;
	entry: ServiceEntry;
	revision: number;
	confirm: boolean;
}

export type CatalogRead = { source: Source } & Read<CatalogData>;
export type RouteRead = RouteKey & Read<RouteData>;

function serviceUrl(source: Source, routes: readonly string[] | null, at: number | undefined) {
	const params = new URLSearchParams();
	if (routes) params.set('route_ids', routes.join(','));
	if (at !== undefined) params.set('at', String(at));
	return `/api/v1/service/${source}${params.size ? `?${params}` : ''}`;
}

/**
 * Layout-scoped owner of catalogs, alerts and passenger service. Views use
 * native snapshots through bindings whose explicit effects own reader interest and first loads.
 */
export class TransitData {
	readonly status = $state({ warning: false, error: null as Error | null });
	#fetcher: typeof fetch;
	#refreshMs: number | undefined;
	#historyLimit: number;
	#batchMs: number;
	#eagerService: boolean;
	#at: number | undefined;
	#sources = new Set<Source>();
	#generation = $state(0);
	#catalogs = new Map<Source, Entry<CatalogData>>();
	#alerts = new Map<string, Entry<AlertsData>>();
	#tracked = new Set<TrackedUpdate>();
	#services = new Map<string, ServiceEntry>();
	#queue = new Map<string, Set<ServiceEntry>>();
	#flushTimers = new Map<string, ReturnType<typeof setTimeout>>();
	#controllers = new Map<Source, AbortController>();
	// Fetch completions may outlive the layout that owns this instance.
	#destroyed = false;
	#cleanup: (() => void)[] = [];

	constructor(options: TransitDataOptions) {
		this.#fetcher = options.fetcher ?? ((...args) => fetch(...args));
		this.#refreshMs = options.refreshMs;
		this.#historyLimit = options.historyLimit ?? 6;
		this.#batchMs = options.batchMs ?? 200;
		this.#eagerService = options.eagerService ?? true;
		this.#at = options.at;
		untrack(() => this.setSources(options.sources));
		if (!browser) return;
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
			if (!browser) break;
			if (this.#sources.has(source)) continue;
			const catalog = new Entry<CatalogData>(source, undefined, null, 'catalog');
			catalog.eager = true;
			this.#catalogs.set(source, catalog);
			void this.#loadCatalog(catalog);
		}
		if (next.size !== this.#sources.size || [...next].some((source) => !this.#sources.has(source)))
			this.#generation++;
		this.#sources = next;
		this.#background();
	}

	/** Enable whole-source background service outside the map, keeping acquired readers active. */
	setEagerService(enabled: boolean) {
		if (enabled === this.#eagerService) return;
		this.#eagerService = enabled;
		this.#background();
	}

	/** Move background readers to the layout's passenger time. */
	setTime(at: number | undefined) {
		if (at === this.#at) return;
		this.#at = at;
		this.#background();
	}

	#background() {
		for (const entry of [...this.#alerts.values(), ...this.#services.values()]) entry.eager = false;
		if (!browser) return;
		for (const source of this.#sources) {
			const alerts = this.#alertEntry(source, this.#at);
			alerts.eager = true;
			this.#ensure(alerts);
			if (!this.#eagerService || !sourceConfig[source].wholeService) continue;
			const service = this.#service(source, null, this.#at);
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
					entry.usedAt = Date.now();
					this.#ensure(entry);
				}
				this.#updateStatus();
				return () => {
					for (const entry of entries) entry.readers--;
					this.#updateStatus();
				};
			});
		});
		return {
			get current() {
				return selection.read();
			}
		};
	}

	#catalogEntry(source: Source) {
		let entry = this.#catalogs.get(source);
		if (!entry) this.#catalogs.set(source, (entry = new Entry(source, undefined, null, 'catalog')));
		return entry;
	}

	/** Getters keep reused component props reactive without recreating a binding. */
	catalog(source: () => Source | null): Resource<Read<CatalogData>> {
		return this.#bind(() => {
			const key = source();
			const entry = key === null ? undefined : this.#catalogEntry(key);
			return {
				entries: entry ? [entry] : [],
				read: () => entry?.current ?? { state: 'idle' }
			};
		});
	}

	catalogs(sources: () => readonly Source[]): Resource<CatalogRead[]> {
		return this.#bind(() => {
			const entries = sources().map((source) => this.#catalogEntry(source));
			return {
				entries,
				read: () =>
					entries.map((entry) => ({
						source: entry.source,
						...entry.current
					}))
			};
		});
	}

	alerts(key: () => { source: Source; at?: number } | null): Resource<Read<AlertsData>> {
		return this.#bind(() => {
			const selected = key();
			const entry = selected && this.#alertEntry(selected.source, selected.at);
			return {
				entries: entry ? [entry] : [],
				read: () => entry?.current ?? { state: 'idle' }
			};
		});
	}

	#routeSnapshot(entry: ServiceEntry, routeId: string): Read<RouteData> {
		const current = entry.current;
		if (current.state !== 'ready') return current;
		const route = current.value.get(routeId);
		return route ? { state: 'ready', value: route } : { state: 'missing' };
	}

	route(key: () => RouteKey | null): Resource<Read<RouteData>> {
		return this.#bind(() => {
			const selected = key();
			const entry =
				selected &&
				this.#service(
					selected.source,
					this.#keyRoute(selected.source, selected.routeId),
					selected.at
				);
			return {
				entries: entry ? [entry] : [],
				read: () =>
					selected && entry ? this.#routeSnapshot(entry, selected.routeId) : { state: 'idle' }
			};
		});
	}

	routes(keys: () => readonly RouteKey[]): Resource<RouteRead[]> {
		return this.#bind(() => {
			const selected = keys().map((key) => ({
				key,
				entry: this.#service(key.source, this.#keyRoute(key.source, key.routeId), key.at)
			}));
			return {
				entries: [...new Set(selected.map(({ entry }) => entry))],
				read: () =>
					selected.map(({ key, entry }) => {
						return { ...key, ...this.#routeSnapshot(entry, key.routeId) };
					})
			};
		});
	}

	trip(key: () => TripKey | null): Resource<Read<TripRead>> {
		const trips = this.trips(() => {
			const requested = key();
			return requested ? [requested] : [];
		});
		return {
			get current() {
				return trips.current[0] ?? { state: 'idle' };
			}
		};
	}

	trips(keys: () => readonly TripKey[]): Resource<(TripKey & Read<TripRead>)[]> {
		let acquired = new Map<string, TripSelection>();
		const selected = $derived.by(() => {
			void this.#generation;
			const requested = keys();
			return untrack(() => {
				const next = new Map<string, TripSelection>();
				const selections = requested.map((requested) => {
					const key = JSON.stringify([
						requested.source,
						requested.routeId,
						requested.tripId,
						requested.at
					]);
					const entry = this.#service(
						requested.source,
						this.#keyRoute(requested.source, requested.routeId),
						requested.at
					);
					const previous = acquired.get(key);
					// Preserve confirmation identity when sibling keys are added or removed.
					const selection =
						previous?.entry === entry
							? previous
							: {
									requested,
									entry,
									revision: entry.revision,
									confirm:
										requested.at === undefined &&
										entry.snapshot.state !== 'loading' &&
										!entry.data?.get(requested.routeId)?.trips.has(requested.tripId)
								};
					next.set(key, selection);
					return selection;
				});
				acquired = next;
				return selections;
			});
		});
		const resource = this.#bind(() => {
			const selections = selected;
			return {
				entries: [...new Set(selections.map(({ entry }) => entry))],
				read: () =>
					selections.map((selection) => ({
						...selection.requested,
						...this.#tripSnapshot(selection)
					}))
			};
		});
		$effect(() => {
			const selections = selected;
			untrack(() => {
				for (const { entry, revision, confirm } of selections)
					if (confirm && entry.revision === revision) this.#start(entry);
			});
		});
		return resource;
	}

	#tripSnapshot({ requested, entry, revision, confirm }: TripSelection): Read<TripRead> {
		const current = this.#routeSnapshot(entry, requested.routeId);
		const route = current.state === 'ready' ? current.value : undefined;
		const trip = route?.trips.get(requested.tripId);
		if (route && trip)
			return {
				state: 'ready',
				value: { trip, calls: route.callsByTrip.get(trip.id) ?? [], route }
			};
		if (current.state === 'failed' || current.state === 'loading') return current;
		if (confirm && entry.revision === revision)
			return entry.error ? { state: 'failed', error: entry.error } : { state: 'loading' };
		return { state: 'missing' };
	}

	stopArrivals(
		key: () => { source: Source; stopId: string; at?: number }
	): Resource<Read<StopArrivals>> {
		return this.#bind(() => {
			const { source, stopId, at } = key();
			const catalog = this.#catalogEntry(source);
			const ids = [
				...new Set(catalog.data?.stopsById.get(stopId)?.routes.map((route) => route.route_id))
			];
			const selected = ids.map((id) => ({
				id,
				entry: this.#service(source, this.#keyRoute(source, id), at)
			}));
			return {
				entries: [catalog, ...new Set(selected.map(({ entry }) => entry))],
				read: () => {
					const catalogRead = catalog.current;
					if (catalogRead.state !== 'ready') return catalogRead;
					if (!catalogRead.value.stopsById.has(stopId)) return { state: 'missing' };
					const routes = new Map<string, RouteData>();
					const pending: string[] = [];
					let error: Error | undefined;
					for (const { id, entry } of selected) {
						const current = this.#routeSnapshot(entry, id);
						if (current.state === 'ready') routes.set(id, current.value);
						else if (current.state === 'loading') pending.push(id);
						else if (current.state === 'failed') error ??= current.error;
					}
					if (!routes.size && pending.length) return { state: 'loading' };
					if (!routes.size && error) return { state: 'failed', error };
					return {
						state: 'ready',
						value: { calls: callsAtStop(routes.values(), stopId), routes, pending }
					};
				}
			};
		});
	}

	/** Pure peeks: observe available snapshots without acquiring service interest. */
	peekCatalog(source: Source): CatalogData | undefined {
		void this.#generation;
		return this.#catalogs.get(source)?.data;
	}

	loadedStopArrivals(
		source: Source,
		stopId: string,
		at: number | undefined = undefined
	): Pick<StopArrivals, 'calls' | 'routes'> {
		const catalog = this.peekCatalog(source);
		const routes = new Map<string, RouteData>();
		for (const { route_id: routeId } of catalog?.stopsById.get(stopId)?.routes ?? []) {
			const route = this.#peek(source, routeId, at);
			if (route) routes.set(routeId, route);
		}
		return { calls: callsAtStop(routes.values(), stopId), routes };
	}

	loadedTrip(
		source: Source,
		routeId: string,
		tripId: string,
		at: number | undefined = undefined
	): TripRead | undefined {
		const route = this.#peek(source, routeId, at);
		const trip = route?.trips.get(tripId);
		return route && trip && { trip, calls: route.callsByTrip.get(tripId) ?? [], route };
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
		const entry = this.#catalogEntry(body.source);
		let catalog: CatalogData;
		try {
			catalog = decodeCatalog(body);
		} catch (error) {
			entry.fail(toError(error), true);
			this.#updateStatus();
			return;
		}
		entry.install(catalog, Date.now());
		for (const entry of this.#services.values())
			if (
				entry.source === body.source &&
				entry.active &&
				entry.snapshot.state === 'missing' &&
				entry.routeId !== null &&
				catalog.routesById.has(entry.routeId)
			)
				this.#start(entry);
		this.#updateStatus();
	}

	#ensure(entry: Entry<unknown>) {
		if (this.#destroyed || entry.inFlight) return;
		if (entry.snapshot.state !== 'loading' && (entry.kind === 'catalog' || entry.at !== undefined))
			return;
		if (
			(entry.snapshot.state !== 'loading' || entry.error) &&
			(this.#due(entry) > Date.now() || !this.#mayPoll())
		)
			return;
		this.#start(entry);
	}

	#start(entry: Entry<unknown>) {
		if (this.#destroyed || entry.inFlight) return;
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
			if (!entry.active && !entry.eager) {
				if (entry.fatal) entry.reset();
				continue;
			}
			if (entry.kind === 'catalog' ? !entry.error : entry.at !== undefined && !entry.error)
				continue;
			this.#start(entry);
		}
		for (const update of this.#tracked) if (update.active) update.retry();
		this.#updateStatus();
	}

	/** Stops timers and requests without touching reactive state that unmounting views may read. */
	destroy() {
		this.#destroyed = true;
		for (const cleanup of this.#cleanup) cleanup();
		for (const controller of this.#controllers.values()) controller.abort();
		this.#queue.clear();
		for (const timer of this.#flushTimers.values()) clearTimeout(timer);
		this.#flushTimers.clear();
	}

	#keyRoute(source: Source, routeId: string) {
		return sourceConfig[source].wholeService ? null : routeId;
	}

	#service(source: Source, routeId: string | null, at: number | undefined): ServiceEntry {
		const key = `${source}|${at ?? ''}|${routeId ?? '*'}`;
		let entry = this.#services.get(key);
		if (!entry) this.#services.set(key, (entry = new Entry(source, at, routeId)));
		return entry;
	}

	#peek(source: Source, routeId: string, at: number | undefined): RouteData | undefined {
		void this.#generation;
		const entry = this.#service(source, this.#keyRoute(source, routeId), at);
		if (!entry.data) return undefined;
		return entry.data.get(routeId);
	}

	#alertEntry(source: Source, at: number | undefined) {
		const key = `${source}|${at ?? ''}`;
		let entry = this.#alerts.get(key);
		if (!entry) this.#alerts.set(key, (entry = new Entry(source, at, null, 'alerts')));
		return entry;
	}

	#due(entry: Pick<ServiceEntry, 'error' | 'attemptedAt' | 'failures' | 'fetchedAt' | 'source'>) {
		if (entry.error && entry.attemptedAt !== null) {
			const backoff = FIRST_RETRY_MS * 2 ** Math.max(0, entry.failures - 1);
			return (
				entry.attemptedAt +
				Math.min(this.#refreshMs ?? sourceConfig[entry.source].refreshMs, backoff)
			);
		}
		return (entry.fetchedAt ?? 0) + (this.#refreshMs ?? sourceConfig[entry.source].refreshMs);
	}

	#mayPoll() {
		if (!browser) return false;
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
		const query = entry.at === undefined ? '' : `?at=${entry.at}`;
		return this.#load(
			entry,
			`/api/v1/alerts/${entry.source}${query}`,
			{ cache: 'no-store' },
			(body: ApiAlert[]) => decodeAlerts(body)
		);
	}

	async #load<B, T>(entry: Entry<T>, url: string, init: RequestInit, decode: (body: B) => T) {
		// Server renders show pending UI; the per-request owner never fetches.
		if (!browser || this.#destroyed) return;
		const { signal } = this.#controller(entry.source);
		entry.attemptedAt = Date.now();
		entry.inFlight = true;
		const revision = entry.revision;
		let decoding = false;
		try {
			const response = await this.#fetcher(url, {
				...init,
				signal: AbortSignal.any([signal, AbortSignal.timeout(15_000)])
			});
			if (!response.ok) throw new Error(`${url} returned ${response.status}`);
			const body = (await response.json()) as B;
			decoding = true;
			if (!signal.aborted && this.#owns(entry) && entry.revision === revision)
				entry.install(decode(body), Date.now());
		} catch (error) {
			if (!signal.aborted && this.#owns(entry) && entry.revision === revision)
				entry.fail(toError(error), decoding);
		} finally {
			entry.inFlight = false;
			this.#evictHistory();
			this.#updateStatus();
		}
	}

	#enqueue(entry: ServiceEntry) {
		if (!browser || this.#destroyed) return;
		const group = `${entry.source}|${entry.at ?? ''}`;
		let entries = this.#queue.get(group);
		if (!entries) this.#queue.set(group, (entries = new Set()));
		// Duplicate readers and polling ticks must not postpone an existing batch.
		if (entries.has(entry)) return;
		entries.add(entry);
		clearTimeout(this.#flushTimers.get(group));
		// Whole-source subway requests do not wait for scrolling bus rows.
		const delay = entry.routeId === null ? Math.min(10, this.#batchMs) : this.#batchMs;
		this.#flushTimers.set(
			group,
			setTimeout(() => this.#flush(group), delay)
		);
	}

	#flush(group: string) {
		this.#flushTimers.delete(group);
		const queued = this.#queue.get(group);
		this.#queue.delete(group);
		if (!queued) return;
		const entries = [...queued].filter(
			(entry) =>
				(entry.active || entry.eager) &&
				!entry.inFlight &&
				this.#services.get(this.#keyOf(entry)) === entry
		);
		if (!entries.length) return;
		if (entries[0].routeId === null) {
			void this.#request(entries.slice(0, 1));
			return;
		}
		entries.sort((a, b) => a.routeId!.localeCompare(b.routeId!));
		for (let i = 0; i < entries.length; i += MAX_BATCH_ROUTES) {
			void this.#request(entries.slice(i, i + MAX_BATCH_ROUTES));
		}
	}

	#keyOf(entry: ServiceEntry) {
		return `${entry.source}|${entry.at ?? ''}|${entry.routeId ?? '*'}`;
	}

	async #request(entries: ServiceEntry[]) {
		const { source, at } = entries[0];
		const routes = entries[0].routeId === null ? null : entries.map((entry) => entry.routeId!);
		const { signal } = this.#controller(source);
		const attempted = Date.now();
		for (const entry of entries) {
			entry.inFlight = true;
			entry.attemptedAt = attempted;
		}
		let decoding = false;
		try {
			const response = await this.#fetcher(serviceUrl(source, routes, at), {
				signal: AbortSignal.any([signal, AbortSignal.timeout(15_000)]),
				cache: 'no-store'
			});
			if (signal.aborted || this.#destroyed) return;
			// A saved catalog can name a retired route; isolate it so valid batch peers still load.
			if (response.status === 404 && routes) {
				const body = (await response.json()) as UnknownRoutes;
				decoding = true;
				const unknown = new Set(body.unknown_route_ids);
				if (!unknown.size) throw new Error('Service 404 omitted unknown route IDs');
				if (signal.aborted || this.#destroyed) return;
				for (const entry of entries) {
					if (!this.#owns(entry)) continue;
					entry.inFlight = false;
					if (!unknown.has(entry.routeId!)) this.#enqueue(entry);
					else entry.markMissing(Date.now());
				}
				return;
			}
			if (!response.ok) throw new Error(`Service returned ${response.status}`);
			const service = (await response.json()) as Service;
			decoding = true;
			const now = Date.now();
			const decoded = decodeService(service, now);
			for (const entry of entries) {
				if (signal.aborted || !this.#owns(entry)) continue;
				entry.inFlight = false;
				entry.sourceUpdatedAt =
					at === undefined && service.updated_at != null ? service.updated_at * 1000 : null;
				if (routes === null) entry.install(decoded, now);
				else {
					const route = decoded.get(entry.routeId!);
					if (route) entry.install(new Map([[entry.routeId!, route]]), now);
					else entry.fail(new Error(`Service omitted route ${entry.routeId}`), true);
				}
			}
		} catch (error) {
			if (signal.aborted) return;
			for (const entry of entries) {
				if (signal.aborted || !this.#owns(entry)) continue;
				entry.inFlight = false;
				entry.fail(toError(error), decoding);
			}
		} finally {
			this.#evictHistory();
			this.#updateStatus();
		}
	}

	#evictHistory() {
		type Evictable = Pick<ServiceEntry, 'at' | 'active' | 'eager' | 'inFlight' | 'usedAt'>;
		for (const entries of [this.#services, this.#alerts] as Map<string, Evictable>[]) {
			const evictable = [...entries.entries()]
				.filter(
					([, entry]) => entry.at !== undefined && !entry.active && !entry.eager && !entry.inFlight
				)
				.sort(([, a], [, b]) => a.usedAt - b.usedAt);
			const historical = [...entries.values()].filter((entry) => entry.at !== undefined).length;
			for (const [key] of evictable.slice(0, Math.max(0, historical - this.#historyLimit))) {
				entries.delete(key);
			}
		}
	}

	#tick() {
		this.#updateStatus();
		if (!this.#mayPoll()) return;
		for (const entry of [
			...this.#catalogs.values(),
			...this.#services.values(),
			...this.#alerts.values()
		]) {
			if (!entry.active && !entry.eager) continue;
			if (entry.kind === 'catalog' ? !entry.error : entry.at !== undefined) continue;
			this.#ensure(entry);
		}
		this.#evictHistory();
	}

	#updateStatus() {
		if (this.#destroyed) return;
		let warning = browser && !navigator.onLine;
		const now = Date.now();
		let error: Error | null = null;
		let catalogAvailable = false;
		let catalogPending = false;
		let catalogError: Error | null = null;
		for (const entry of [
			...this.#catalogs.values(),
			...this.#services.values(),
			...this.#alerts.values()
		]) {
			if (!entry.active && !entry.eager) continue;
			if (entry.error) warning = true;
			if (entry.fatal) error ??= entry.error;
			if (entry.kind === 'catalog') {
				catalogAvailable ||= entry.current.state === 'ready';
				catalogPending ||= entry.current.state === 'loading';
				catalogError ??= entry.error;
			}
			if (
				entry.snapshot.state === 'missing' ||
				entry.kind === 'catalog' ||
				entry.at !== undefined ||
				entry.fetchedAt === null
			)
				continue;
			const refreshed = Math.min(entry.fetchedAt, entry.sourceUpdatedAt ?? entry.fetchedAt);
			// TODO: If revisiting stale rows causes distracting warning flashes, consider
			// waiting for their refresh result before showing the freshness warning.
			if (now - refreshed > STALE_AFTER_MS) warning = true;
		}
		for (const update of this.#tracked) {
			if (!update.active) continue;
			if (update.fatal) error ??= update.error;
			if (
				update.error ||
				(update.at == null &&
					update.fetchedAt !== null &&
					now - Math.min(update.fetchedAt, update.sourceUpdatedAt ?? update.fetchedAt) >
						STALE_AFTER_MS)
			)
				warning = true;
		}
		this.status.warning = warning;
		this.status.error = error ?? (!catalogAvailable && !catalogPending ? catalogError : null);
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
			clearTimeout(this.#flushTimers.get(group));
			this.#flushTimers.delete(group);
		}
	}
}

export const [getTransitData, setTransitData] = createContext<TransitData>();
