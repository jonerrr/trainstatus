import type { Source } from '#lib/client/index.js';
import type { CatalogData } from '#lib/data/decode.js';

import type { Feature, FeatureCollection, Point } from 'geojson';

export interface StopProperties {
	id: string;
	source: Source;
	name: string;
	north_headsign?: string;
	south_headsign?: string;
	direction?: string;
}

/** Build stop features from already-loaded catalogs (no extra network I/O). */
export function buildStopsGeoJSON(
	sources: readonly Source[],
	catalogFor: (source: Source) => CatalogData | undefined
): FeatureCollection<Point, StopProperties> {
	const features: Feature<Point, StopProperties>[] = [];
	for (const source of sources) {
		const catalog = catalogFor(source);
		if (!catalog) continue;
		for (const stop of catalog.stops) {
			const properties: StopProperties = {
				id: stop.id,
				source,
				name: stop.name
			};
			if (stop.data.source === 'mta_subway') {
				properties.north_headsign = stop.data.north_headsign;
				properties.south_headsign = stop.data.south_headsign;
			} else if (stop.data.source === 'mta_bus') {
				properties.direction = stop.data.direction;
			}
			features.push({
				type: 'Feature',
				id: stop.id,
				properties,
				geometry: { type: 'Point', coordinates: [stop.lon, stop.lat] }
			});
		}
	}
	return { type: 'FeatureCollection', features };
}

export type StopsGeoJSON = FeatureCollection<Point, StopProperties>;
