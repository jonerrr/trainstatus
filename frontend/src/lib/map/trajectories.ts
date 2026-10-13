import type { Source } from '#lib/client/index.js';
import type { TrackedUpdate } from '#lib/transit/store.svelte.js';

import { renderUnitTableFromIPC, type RenderUnitTable } from './trajectoryArrow';

const FIRST_RETRY_MS = 5_000;

export interface TrajectoryQuery {
	sources: readonly Source[];
	at: number | null;
	refreshInterval: number;
}
export interface TrajectoryStatus {
	available: boolean;
	error: Error | null;
}
export interface TrajectorySnapshot {
	tables: ReadonlyMap<Source, RenderUnitTable>;
	statuses: ReadonlyMap<Source, TrajectoryStatus>;
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
	const statuses = new Map<Source, TrajectoryStatus>();
	const publish = () => {
		if (!controller.signal.aborted)
			onchange({ tables: new Map(tables), statuses: new Map(statuses) });
	};

	const pollers = query.sources.map((source) => {
		const params = query.at === null ? '' : `?at=${query.at}`;
		const url = `/api/v1/trajectories/${source}${params}`;
		let timer: ReturnType<typeof setTimeout> | undefined;
		let failures = 0;
		let in_flight: Promise<void> | undefined;
		// The shared status reads this on its next tick.
		const update = {
			active: true as boolean,
			at: query.at,
			error: null as Error | null,
			fetched_at: null as number | null,
			retry: () => void load()
		};
		statuses.set(source, { available: false, error: null });

		async function request() {
			try {
				const response = await fetcher(url, {
					signal: AbortSignal.any([controller.signal, AbortSignal.timeout(15_000)]),
					cache: 'no-store'
				});
				if (!response.ok) throw new Error(`Trajectory service returned ${response.status}`);
				const table = renderUnitTableFromIPC(await response.arrayBuffer());
				if (controller.signal.aborted) return;
				tables.set(source, table);
				failures = 0;
				update.error = null;
				update.fetched_at = Date.now();
				statuses.set(source, { available: true, error: null });
				if (query.at === null) schedule(query.refreshInterval);
			} catch (error) {
				if (controller.signal.aborted) return;
				failures++;
				update.error = error as Error;
				statuses.set(source, { available: tables.has(source), error: error as Error });
				schedule(Math.min(query.refreshInterval, FIRST_RETRY_MS * 2 ** (failures - 1)));
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
					schedule(query.refreshInterval);
				else void load();
			}, delay);
		}

		const untrack = track?.(update);
		return {
			load,
			stop() {
				clearTimeout(timer);
				update.active = false;
				untrack?.();
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
