/* eslint-disable svelte/prefer-svelte-reactivity -- pollers publish immutable map
   snapshots through native state; their private working maps are not reactive. */
import { onDestroy, untrack } from 'svelte';

import type { Source } from '#lib/client/index.js';
import { source_config } from '#lib/sources.js';
import { toError } from '#lib/transit/resource.svelte.js';
import { getTransitData, type TrackedUpdate } from '#lib/transit/store.svelte.js';

import { renderUnitTableFromIPC, type RenderUnitTable } from './trajectory';

const FIRST_RETRY_MS = 5_000;

export interface TrajectoryQuery {
	sources: readonly Source[];
	at: number | undefined;
	refreshInterval?: number;
}
export interface TrajectorySnapshot {
	tables: ReadonlyMap<Source, RenderUnitTable>;
}

/** Independent source requests retain loaded data until their own query changes. */
export function watchTrajectories(
	query: TrajectoryQuery,
	onchange: (snapshot: TrajectorySnapshot) => void,
	fetcher: typeof fetch = (...args) => fetch(...args),
	track?: (update: TrackedUpdate) => () => void
) {
	const tables = new Map<Source, RenderUnitTable>();
	const pollers = new Map<Source, ReturnType<typeof watchSource>>();
	let destroyed = false;
	const publish = () => onchange({ tables: new Map(tables) });

	function update(next: TrajectoryQuery) {
		if (destroyed) return;
		for (const [source, poller] of pollers) {
			if (
				!next.sources.includes(source) ||
				poller.at !== next.at ||
				poller.refresh_ms !== (next.refreshInterval ?? source_config[source].refresh_ms)
			) {
				poller.destroy();
				pollers.delete(source);
				tables.delete(source);
			}
		}
		for (const source of next.sources) {
			if (pollers.has(source)) continue;
			pollers.set(
				source,
				watchSource(
					source,
					next.at,
					next.refreshInterval ?? source_config[source].refresh_ms,
					(table) => {
						tables.set(source, table);
						publish();
					},
					fetcher,
					track
				)
			);
		}
		publish();
	}
	update(query);
	return {
		update,
		async refresh() {
			await Promise.all([...pollers.values()].map((poller) => poller.load()));
		},
		destroy() {
			destroyed = true;
			for (const poller of pollers.values()) poller.destroy();
			pollers.clear();
		}
	};
}

function watchSource(
	source: Source,
	at: number | undefined,
	refresh_ms: number,
	onchange: (table: RenderUnitTable) => void,
	fetcher: typeof fetch,
	track?: (update: TrackedUpdate) => () => void
) {
	const controller = new AbortController();
	const url = `/api/v1/trajectories/${source}${at === undefined ? '' : `?at=${at}`}`;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let failures = 0;
	let in_flight: Promise<void> | undefined;
	// The shared status reads this on its next tick.
	const update = {
		active: true as boolean,
		at,
		error: null as Error | null,
		fatal: false,
		fetched_at: null as number | null,
		source_updated_at: null as number | null,
		retry() {
			if (update.fatal) {
				update.error = null;
				update.fatal = false;
			}
			void load();
		}
	};
	async function request() {
		let decoding = false;
		try {
			const response = await fetcher(url, {
				signal: AbortSignal.any([controller.signal, AbortSignal.timeout(15_000)]),
				cache: 'no-store'
			});
			if (!response.ok) throw new Error(`Trajectory service returned ${response.status}`);
			const bytes = await response.arrayBuffer();
			if (controller.signal.aborted) return;
			decoding = true;
			const table = renderUnitTableFromIPC(bytes);
			failures = 0;
			update.error = null;
			update.fatal = false;
			update.fetched_at = Date.now();
			const committed_at = response.headers.get('x-source-updated-at');
			update.source_updated_at =
				at === undefined && committed_at !== null ? Number(committed_at) * 1000 : null;
			if (at === undefined) schedule(refresh_ms);
			onchange(table);
		} catch (error) {
			if (controller.signal.aborted) return;
			failures++;
			update.error = toError(error);
			update.fatal = decoding;
			schedule(Math.min(refresh_ms, FIRST_RETRY_MS * 2 ** (failures - 1)));
		}
	}
	function load() {
		if (controller.signal.aborted) return Promise.resolve();
		clearTimeout(timer);
		in_flight ??= request().finally(() => (in_flight = undefined));
		return in_flight;
	}
	function schedule(delay: number) {
		clearTimeout(timer);
		timer = setTimeout(() => {
			if (typeof document !== 'undefined' && document.visibilityState === 'hidden')
				schedule(refresh_ms);
			else void load();
		}, delay);
	}
	const unregister = track?.(update);
	void load();
	return {
		at,
		refresh_ms,
		load,
		destroy() {
			controller.abort();
			clearTimeout(timer);
			update.active = false;
			unregister?.();
		}
	};
}

/** Component-owned binding; reads do not acquire requests. */
export function createTrajectories(query: () => TrajectoryQuery) {
	const transit = getTransitData();
	let snapshot = $state.raw<TrajectorySnapshot>({ tables: new Map() });
	let watcher: ReturnType<typeof watchTrajectories> | undefined;
	$effect(() => {
		const selected = query();
		const next = { ...selected, sources: [...selected.sources] };
		untrack(() => {
			if (watcher) watcher.update(next);
			else
				watcher = watchTrajectories(
					next,
					(value) => {
						snapshot = value;
					},
					undefined,
					(update) => transit.track(update)
				);
		});
	});
	onDestroy(() => watcher?.destroy());
	return {
		get current() {
			return snapshot;
		}
	};
}
