/* eslint-disable svelte/prefer-svelte-reactivity -- maps only deduplicate request keys
   during derivation; bindings observe native snapshots. */
import type { CatalogStop, Source } from '#lib/client/index.js';

import {
	boardingOptions,
	compareTransfers,
	departureOptions,
	type BoardingOption,
	type DepartureOption,
	type IncomingTrip
} from './nearby.js';
import { getTransitData, type Read, type Resource } from './store.svelte.js';

interface TransferQuery {
	stop: CatalogStop;
	incoming?: IncomingTrip;
	sources: readonly Source[];
	at: number | undefined;
	now: number;
	open: boolean;
}

type TransferRead = Read<DepartureOption[]> & { options: BoardingOption[] };

/** One lifecycle for catalog discovery, batched service and transfer outcomes. */
export function createTransfers(query: () => TransferQuery): Resource<TransferRead> {
	const transit = getTransitData();
	const { stop, incoming, sources, at, now, open } = $derived(query());
	const catalogsResource = transit.catalogs(() =>
		sources.filter(
			(source) => source === stop.data.source || stop.nearby_stops.some((s) => s.source === source)
		)
	);
	const catalogReads = $derived(catalogsResource.current);
	const catalogs = $derived(catalogReads.flatMap((r) => (r.state === 'ready' ? [r.value] : [])));
	const options = $derived(boardingOptions(stop, catalogs, incoming));
	const routesResource = transit.routes(() =>
		open
			? [
					...new Map(
						options.map((o) => [
							JSON.stringify([o.route.data.source, o.route.id]),
							{ source: o.route.data.source, routeId: o.route.id, at }
						])
					).values()
				]
			: []
	);
	const reads = $derived(routesResource.current);
	const value = $derived(
		open ? departureOptions(options, reads, incoming, catalogs, now).sort(compareTransfers) : []
	);
	return {
		get current(): TransferRead {
			if (!open) return { state: 'idle', options };
			if (value.length) return { state: 'ready', value, options };
			if (
				catalogReads.some((r) => r.state === 'loading') ||
				reads.some((r) => r.state === 'loading')
			)
				return { state: 'loading', options };
			const failure =
				catalogReads.find((r) => r.state === 'failed') ?? reads.find((r) => r.state === 'failed');
			return failure ? { ...failure, options } : { state: 'ready', value, options };
		}
	};
}
