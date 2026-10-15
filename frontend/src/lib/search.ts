import type { CatalogStop } from '#lib/client/index.js';
import type { CatalogData } from '#lib/transit/decode.js';

import { Index } from 'flexsearch';

// A replaced catalog is a new object, so its index is rebuilt on first search.
const indexes = new WeakMap<CatalogData, Index>();

/** Stops whose names match the term; every stop when the term is blank. */
export function search_stops(catalog: CatalogData, term: string): readonly CatalogStop[] {
	if (!term.trim()) return catalog.stops;
	let index = indexes.get(catalog);
	if (!index) {
		index = new Index({ tokenize: 'forward' });
		catalog.stops.forEach((stop, i) => index!.add(i, stop.name));
		indexes.set(catalog, index);
	}
	return index.search(term).map((i) => catalog.stops[i as number]);
}
