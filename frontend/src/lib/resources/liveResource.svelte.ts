import { untrack } from 'svelte';

import { resourceQuery, type ResourceQuery } from './request';
import { getUpdateStatus, type UpdateStatus } from './status.svelte';

export interface ResourceSnapshot<T> {
	current: T;
	query: ResourceQuery | null;
	dataQuery: ResourceQuery | null;
	available: boolean;
	fetching: boolean;
	error: Error | null;
}
function cancelled(): Error {
	return new DOMException('Request cancelled', 'AbortError');
}

interface Waiter<T> {
	query: ResourceQuery;
	route?: string;
	resolve: (value: T) => void;
	reject: (error: Error) => void;
}
export interface LiveResourceOptions<T> {
	query?: () => ResourceQuery;
	updates?: UpdateStatus;
	onchange?: (snapshot: ResourceSnapshot<T>) => void;
	interval?: number;
	debounce?: number;
	retain?: (data: T, previous: ResourceQuery, next: ResourceQuery) => T;
}
/** Owns reactive data, query generations, polling, and request promises. */
export class LiveResource<T> {
	snapshot: ResourceSnapshot<T>;
	#initial: T;
	#fetcher: (query: ResourceQuery, signal: AbortSignal) => Promise<T>;
	#unregister: (() => void) | undefined;
	#options: LiveResourceOptions<T>;
	#generation = 0;
	#disposed = false;
	#timer: ReturnType<typeof setTimeout> | undefined;
	#debounce: ReturnType<typeof setTimeout> | undefined;
	#request: AbortController | undefined;
	#waiters: Waiter<T>[] = [];
	constructor(
		fetcher: (query: ResourceQuery, signal: AbortSignal) => Promise<T>,
		initial: T,
		options: LiveResourceOptions<T> = {}
	) {
		this.#fetcher = fetcher;
		this.#initial = initial;
		this.#options = { ...options };
		this.snapshot = $state.raw({
			current: initial,
			query: null,
			dataQuery: null,
			available: false,
			fetching: false,
			error: null
		});
		const updates = options.updates ?? (options.query ? getUpdateStatus() : undefined);
		this.#unregister = updates?.register(this);
		const getQuery = options.query;
		if (getQuery) {
			$effect(() => {
				const query = getQuery();
				untrack(() => this.setQuery(query));
			});
			$effect(() => () => this.dispose());
		}
	}
	get current() {
		return this.snapshot.current;
	}
	get available() {
		return this.snapshot.available;
	}
	get fetching() {
		return this.snapshot.fetching;
	}
	get error() {
		return this.snapshot.error;
	}
	get active() {
		return this.snapshot.query?.active ?? false;
	}
	protected syncQuery() {
		const query = this.#options.query;
		if (!this.#disposed && query) untrack(() => this.setQuery(query()));
	}
	#update(patch: Partial<ResourceSnapshot<T>>) {
		this.snapshot = { ...this.snapshot, ...patch };
		this.#options.onchange?.(this.snapshot);
	}
	setQuery(query: ResourceQuery) {
		if (this.#disposed || this.snapshot.query?.key === query.key) return;
		const previous = this.snapshot.dataQuery;
		this.#generation++;
		this.#request?.abort();
		this.#request = undefined;
		clearTimeout(this.#timer);
		clearTimeout(this.#debounce);
		this.#debounce = undefined;
		this.#settle(
			(w) =>
				!w.route ||
				w.query.source !== query.source ||
				w.query.kind !== query.kind ||
				w.query.at !== query.at ||
				!query.active ||
				!query.routes.includes(w.route),
			cancelled()
		);
		const retain =
			previous &&
			previous.at === query.at &&
			previous.source === query.source &&
			previous.kind === query.kind &&
			this.#options.retain;
		this.#update({
			query,
			fetching: false,
			available:
				!!retain && query.active && query.routes.every((route) => previous.routes.includes(route)),
			error: retain ? this.snapshot.error : null,
			...(retain
				? {
						current: retain(this.snapshot.current, previous, query),
						dataQuery: resourceQuery(
							previous.source,
							previous.kind,
							previous.at,
							previous.routes.filter((route) => query.routes.includes(route)),
							query.active
						)
					}
				: {
						current: this.#initial,
						dataQuery: null
					})
		});
		if (query.active) void this.refresh().catch(() => {});
	}
	#covers(query: ResourceQuery, route?: string) {
		const dataQuery = this.snapshot.dataQuery;
		return (
			!!dataQuery &&
			dataQuery.at === query.at &&
			dataQuery.source === query.source &&
			dataQuery.kind === query.kind &&
			(route ? dataQuery.routes.includes(route) : dataQuery.key === query.key)
		);
	}
	refresh(immediate = false): Promise<T> {
		this.syncQuery();
		const query = this.snapshot.query;
		if (this.#disposed || !query?.active) return Promise.reject(cancelled());
		const promise = new Promise<T>((resolve, reject) =>
			this.#waiters.push({ query, resolve, reject })
		);
		if (!this.#request && this.#debounce === undefined) {
			clearTimeout(this.#timer);
			if (immediate || (this.#options.debounce ?? 0) === 0) void this.#execute();
			else
				this.#debounce = setTimeout(() => {
					this.#debounce = undefined;
					void this.#execute();
				}, this.#options.debounce);
		} else if (immediate && this.#debounce !== undefined) {
			clearTimeout(this.#debounce);
			this.#debounce = undefined;
			void this.#execute();
		}
		return promise;
	}
	whenAvailable(route?: string): Promise<T> {
		this.syncQuery();
		const query = this.snapshot.query;
		if (this.#disposed || !query?.active || (route && !query.routes.includes(route)))
			return Promise.reject(cancelled());
		if (this.#covers(query, route)) return Promise.resolve(this.snapshot.current);
		if (this.snapshot.error && !this.snapshot.fetching && this.#debounce === undefined)
			return Promise.reject(this.snapshot.error);
		return new Promise((resolve, reject) => this.#waiters.push({ query, route, resolve, reject }));
	}
	coversRoute(route: string) {
		return (
			!!this.snapshot.query?.active &&
			this.snapshot.query.routes.includes(route) &&
			this.#covers(this.snapshot.query, route)
		);
	}
	#settle(matches: (waiter: Waiter<T>) => boolean, error?: Error) {
		const pending: Waiter<T>[] = [];
		for (const waiter of this.#waiters) {
			if (!matches(waiter)) {
				pending.push(waiter);
				continue;
			}
			if (error) waiter.reject(error);
			else waiter.resolve(this.snapshot.current);
		}
		this.#waiters = pending;
	}
	async #execute() {
		const query = this.snapshot.query;
		if (this.#disposed || !query?.active || this.#request) return;
		const generation = this.#generation;
		const controller = new AbortController();
		this.#request = controller;
		this.#update({ fetching: true });
		try {
			const result = await this.#fetcher(query, controller.signal);
			if (this.#disposed || generation !== this.#generation || controller.signal.aborted) return;
			this.#update({ current: result, dataQuery: query, available: true, error: null });
			this.#settle((w) => (w.route ? this.#covers(query, w.route) : w.query.key === query.key));
		} catch (error) {
			if (this.#disposed || generation !== this.#generation) return;
			const failure = error instanceof Error ? error : new Error(String(error));
			this.#update({ error: failure });
			this.#settle(
				(w) => (w.route ? query.routes.includes(w.route) : w.query.key === query.key),
				failure
			);
		} finally {
			if (!this.#disposed && generation === this.#generation) {
				this.#request = undefined;
				this.#update({ fetching: false });
				const interval = this.#options.interval ?? 5000;
				if (interval > 0 && (query.at === null || this.snapshot.error)) {
					this.#timer = setTimeout(() => {
						void this.refresh(true).catch(() => {});
					}, interval);
				}
			}
		}
	}
	dispose() {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#unregister?.();
		this.#generation++;
		clearTimeout(this.#timer);
		clearTimeout(this.#debounce);
		this.#request?.abort();
		this.#request = undefined;
		this.#settle(() => true, cancelled());
	}
}
