import type { Source, Stop } from '@trainstatus/client';
import { Index } from 'flexsearch';

export class StopSearch {
	indexes = $state({} as Record<Source, Index>);
	stops = $state({} as Record<Source, Stop[]>);

	constructor(stops_by_source: Record<Source, Stop[]>) {
		for (const [source, stops] of Object.entries(stops_by_source) as [Source, Stop[]][]) {
			const index = new Index({ tokenize: 'forward' });
			stops.forEach((stop, i) => {
				index.add(i, stop.name);
			});
			this.indexes[source] = index;
			this.stops[source] = stops;
		}
	}

	search(search_term: string, source: Source) {
		const index = this.indexes[source];
		const source_stops = this.stops[source] ?? [];
		if (!index || !search_term.trim()) {
			return source_stops;
		}
		const results = index.search(search_term);
		return results.map((i) => source_stops[i as number]);
	}
}
