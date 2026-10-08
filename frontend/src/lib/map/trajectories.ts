import type { Source } from '#lib/client/index.js';

import { renderUnitTableFromIPC, type RenderUnitTable } from './trajectoryArrow';

export interface TrajectoryQuery {
	sources: readonly Source[];
	at: number | null;
	refreshInterval: number;
}

export interface TrajectorySnapshot {
	tables: ReadonlyMap<Source, RenderUnitTable>;
	errors: ReadonlyMap<Source, string>;
}

/** One query owns all its requests and timers. Replacing it cannot publish old data. */
export function watchTrajectories(
	query: TrajectoryQuery,
	onchange: (snapshot: TrajectorySnapshot) => void,
	fetcher: typeof fetch = fetch
): () => void {
	const tables = new Map<Source, RenderUnitTable>();
	const errors = new Map<Source, string>();
	const requests = new Map<Source, AbortController>();
	let disposed = false;
	const publish = () => onchange({ tables: new Map(tables), errors: new Map(errors) });
	async function refresh(source: Source) {
		// Do not starve slow requests by aborting them on every polling interval.
		if (requests.has(source)) return;
		const controller = new AbortController();
		requests.set(source, controller);
		try {
			const suffix = query.at === null ? '' : `?at=${query.at}`;
			const response = await fetcher(`/api/v1/trajectories/${source}${suffix}`, {
				signal: controller.signal
			});
			if (!response.ok) throw new Error(`Vehicle data unavailable (${response.status})`);
			const buffer = await response.arrayBuffer();
			if (disposed) return;
			tables.set(source, renderUnitTableFromIPC(buffer));
			errors.delete(source);
		} catch (error) {
			if (disposed) return;
			errors.set(source, error instanceof Error ? error.message : 'Vehicle data unavailable');
		} finally {
			requests.delete(source);
			if (!disposed) publish();
		}
	}
	publish();
	for (const source of query.sources) void refresh(source);
	const timer =
		query.at === null
			? setInterval(() => {
					for (const source of query.sources) void refresh(source);
				}, query.refreshInterval)
			: undefined;
	return () => {
		disposed = true;
		clearInterval(timer);
		for (const request of requests.values()) request.abort();
		requests.clear();
	};
}
