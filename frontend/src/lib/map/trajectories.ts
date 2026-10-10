import type { Source } from '#lib/client/index.js';
import { LiveResource, type ResourceSnapshot } from '#lib/resources/liveResource.svelte.js';
import { requestData, resourceQuery } from '#lib/resources/request.js';
import type { UpdateStatus } from '#lib/resources/status.svelte.js';

import { renderUnitTableFromIPC, type RenderUnitTable } from './trajectoryArrow';

export interface TrajectoryQuery {
	sources: readonly Source[];
	at: number | null;
	refreshInterval: number;
}
export interface TrajectorySnapshot {
	tables: ReadonlyMap<Source, RenderUnitTable>;
	statuses: ReadonlyMap<Source, ResourceSnapshot<RenderUnitTable | null>>;
}
export interface TrajectoryWatcher {
	(): void;
	refresh(): Promise<void>;
}

/** One query owns all requests; the shared query owner rejects late publication. */
export function watchTrajectories(
	query: TrajectoryQuery,
	onchange: (snapshot: TrajectorySnapshot) => void,
	fetcher: typeof fetch = fetch,
	updates?: UpdateStatus
): TrajectoryWatcher {
	const resources = new Map<Source, LiveResource<RenderUnitTable | null>>();
	const statuses = new Map<Source, ResourceSnapshot<RenderUnitTable | null>>();
	let disposed = false;
	const publish = () => {
		if (disposed) return;
		const tables = new Map<Source, RenderUnitTable>();
		for (const [source, state] of statuses) {
			if (state.current) tables.set(source, state.current);
		}
		onchange({ tables, statuses: new Map(statuses) });
	};
	publish();
	for (const source of query.sources) {
		const resource = new LiveResource<RenderUnitTable | null>(
			(captured, signal) =>
				requestData(
					captured.url,
					async (response) => renderUnitTableFromIPC(await response.arrayBuffer()),
					signal,
					fetcher
				),
			null,
			{
				interval: query.refreshInterval,
				updates,
				onchange: (state) => {
					statuses.set(source, state);
					publish();
				}
			}
		);
		resources.set(source, resource);
		resource.setQuery(resourceQuery(source, 'trajectories', query.at));
	}
	const stop: TrajectoryWatcher = Object.assign(
		() => {
			disposed = true;
			for (const resource of resources.values()) resource.dispose();
		},
		{
			refresh: async () => {
				await Promise.all([...resources.values()].map((resource) => resource.refresh(true)));
			}
		}
	);
	return stop;
}
