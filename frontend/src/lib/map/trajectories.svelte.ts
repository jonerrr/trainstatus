/* eslint-disable svelte/prefer-svelte-reactivity -- pollers publish immutable map
   snapshots through native state; their private working maps are not reactive. */
import { untrack } from 'svelte';

import type { Source } from '#lib/client/index.js';
import { source_config } from '#lib/sources.js';
import { toError } from '#lib/transit/resource.svelte.js';
import { getTransitData, type TrackedUpdate } from '#lib/transit/store.svelte.js';

import { renderUnitTableFromIPC, type RenderUnitTable } from './trajectoryArrow';

const FIRST_RETRY_MS = 5_000;

export interface TrajectoryQuery {
	sources: readonly Source[];
	at: number | undefined;
	refreshInterval?: number;
}
export interface TrajectorySnapshot {
	tables: ReadonlyMap<Source, RenderUnitTable>;
}
export interface TrajectoryWatcher {
	(): void;
	refresh(): Promise<void>;
}

/**
 * Poll each source's Arrow trajectories. Live data refreshes on an interval; fixed-time
 * data loads once. Failures retain the previous table and retry with bounded backoff.
 */
export function watchTrajectories(
	query: TrajectoryQuery,
	onchange: (snapshot: TrajectorySnapshot) => void,
	fetcher: typeof fetch = (...args) => fetch(...args),
	track?: (update: TrackedUpdate) => () => void
): TrajectoryWatcher {
	const controller = new AbortController();
	const tables = new Map<Source, RenderUnitTable>();
	const publish = () => {
		if (!controller.signal.aborted) onchange({ tables: new Map(tables) });
	};

	const pollers = query.sources.map((source) => {
		const refresh_ms = query.refreshInterval ?? source_config[source].refresh_ms;
		const params = query.at === undefined ? '' : `?at=${query.at}`;
		const url = `/api/v1/trajectories/${source}${params}`;
		let timer: ReturnType<typeof setTimeout> | undefined;
		let failures = 0;
		let in_flight: Promise<void> | undefined;
		// The shared status reads this on its next tick.
		const update = {
			active: true as boolean,
			at: query.at,
			error: null as Error | null,
			fatal: false,
			fetched_at: null as number | null,
			source_updated_at: null as number | null,
			retry: () => void load()
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
				decoding = true;
				const table = renderUnitTableFromIPC(bytes);
				if (controller.signal.aborted) return;
				tables.set(source, table);
				failures = 0;
				update.error = null;
				update.fatal = false;
				update.fetched_at = Date.now();
				const committed_at = response.headers.get('x-source-updated-at');
				update.source_updated_at =
					query.at === undefined && committed_at !== null ? Number(committed_at) * 1000 : null;
				if (query.at === undefined) schedule(refresh_ms);
			} catch (error) {
				if (controller.signal.aborted) return;
				failures++;
				update.error = toError(error);
				update.fatal = decoding;
				schedule(Math.min(refresh_ms, FIRST_RETRY_MS * 2 ** (failures - 1)));
			}
			publish();
		}

		function load() {
			clearTimeout(timer);
			in_flight ??= request().finally(() => (in_flight = undefined));
			return in_flight;
		}

		function schedule(delay: number) {
			clearTimeout(timer);
			timer = setTimeout(() => {
				// Hidden pages wait for the next interval rather than polling.
				if (typeof document !== 'undefined' && document.visibilityState === 'hidden')
					schedule(refresh_ms);
				else void load();
			}, delay);
		}
		// Register retry and freshness with the one app warning; release on unmount.
		const unregister = track?.(update);

		return {
			load,
			stop() {
				clearTimeout(timer);
				update.active = false;
				unregister?.();
			}
		};
	});

	publish();
	for (const poller of pollers) void poller.load();

	return Object.assign(
		() => {
			controller.abort();
			for (const poller of pollers) poller.stop();
		},
		{
			refresh: async () => {
				await Promise.all(pollers.map((poller) => poller.load()));
			}
		}
	);
}

/** A layout-owned trajectory binding with the same initialization/read pattern as passenger data. */
export function createTrajectories(query: () => TrajectoryQuery) {
	const transit = getTransitData();
	let snapshot = $state.raw<TrajectorySnapshot>({ tables: new Map() });
	$effect(() => {
		const selected = query();
		const sources = [...selected.sources];
		return untrack(() =>
			watchTrajectories(
				{ ...selected, sources },
				(next) => {
					snapshot = next;
				},
				undefined,
				(update) => transit.track(update)
			)
		);
	});
	return {
		get current() {
			return snapshot;
		}
	};
}
