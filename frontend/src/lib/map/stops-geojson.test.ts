import type { Catalog, Source } from '#lib/client/index.js';
import { decodeCatalog, type CatalogData } from '#lib/data/decode.js';

import { describe, expect, it } from 'vitest';

import { buildStopsGeoJSON } from './stops-geojson.js';

function catalog(body: Partial<Catalog> & Pick<Catalog, 'source'>): CatalogData {
	return decodeCatalog({
		routes: [],
		stops: [],
		...body
	} as Catalog);
}

describe('buildStopsGeoJSON', () => {
	const catalogs = new Map<Source, CatalogData>([
		[
			'mta_subway',
			catalog({
				source: 'mta_subway',
				stops: [
					{
						id: '101',
						name: 'South Ferry',
						lon: -74.01,
						lat: 40.7,
						nearby_stops: [],
						routes: [],
						data: {
							source: 'mta_subway',
							bubble_id: '',
							gtfs_stop_id: '',
							is_major: true,
							line: '1',
							north_headsign: 'Uptown',
							south_headsign: 'Downtown',
							station_group_id: '',
							platform_edges: []
						}
					}
				]
			})
		],
		[
			'mta_bus',
			catalog({
				source: 'mta_bus',
				stops: [
					{
						id: '304913',
						name: 'Broadway/W 42 St',
						lon: -73.99,
						lat: 40.76,
						nearby_stops: [],
						routes: [],
						data: {
							source: 'mta_bus',
							direction: 'ne',
							is_boardable: true
						}
					}
				]
			})
		]
	]);

	it('emits tile-compatible properties for subway and bus stops', () => {
		const collection = buildStopsGeoJSON(['mta_subway', 'mta_bus'], (s) => catalogs.get(s));
		expect(collection.features).toHaveLength(2);
		const subway = collection.features.find((f) => f.properties?.source === 'mta_subway')!;
		expect(subway.properties).toEqual({
			id: '101',
			source: 'mta_subway',
			name: 'South Ferry',
			north_headsign: 'Uptown',
			south_headsign: 'Downtown'
		});
		expect(subway.geometry.coordinates).toEqual([-74.01, 40.7]);

		const bus = collection.features.find((f) => f.properties?.source === 'mta_bus')!;
		expect(bus.properties).toMatchObject({ direction: 'ne' });
	});

	it('skips sources with no loaded catalog', () => {
		const collection = buildStopsGeoJSON(['njt_bus'], () => undefined);
		expect(collection.features).toEqual([]);
	});
});
