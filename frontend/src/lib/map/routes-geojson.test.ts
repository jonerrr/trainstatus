import type { Catalog, Source } from '#lib/client/index.js';
import { decodeCatalog, type CatalogData } from '#lib/data/decode.js';

import { describe, expect, it } from 'vitest';

import { buildActiveRouteShapesGeoJSON } from './routes-geojson.js';
import { trajectoryFixture } from './trajectory-fixture.js';
import { renderUnitTableFromIPC } from './trajectory.js';

function catalog(body: Partial<Catalog> & Pick<Catalog, 'source'>): CatalogData {
	return decodeCatalog({
		stops: [],
		...body
	} as Catalog);
}

describe('buildActiveRouteShapesGeoJSON', () => {
	const subway = catalog({
		source: 'mta_subway',
		routes: [
			{
				id: 'A',
				short_name: 'A',
				long_name: 'A Train',
				color: '#0039A6',
				text_color: '#FFFFFF',
				data: { source: 'mta_subway' }
			}
		]
	});
	const bus = catalog({
		source: 'mta_bus',
		routes: [
			{
				id: 'M42',
				short_name: 'M42',
				long_name: 'Crosstown',
				color: '#00AEEF',
				text_color: '#000000',
				data: {
					source: 'mta_bus',
					directions: [],
					name_number: 42,
					name_prefix: 'M',
					service_types: ['SHUTTLE'],
					sort_key: 0
				}
			}
		]
	});

	it('builds one line per active unit shape with route metadata', () => {
		const tables = new Map<Source, ReturnType<typeof renderUnitTableFromIPC>>([
			['mta_subway', renderUnitTableFromIPC(trajectoryFixture(['v1'], 200))],
			[
				'mta_bus',
				renderUnitTableFromIPC(
					trajectoryFixture(['b1'], 200, {
						source: 'mta_bus',
						positions: [
							[-74, 40.75],
							[-73.99, 40.76]
						],
						distances: [0, 120]
					})
				)
			]
		]);
		// Override route ids to match catalogs
		for (const unit of tables.get('mta_bus')!.units) unit.routeId = 'M42';

		const collection = buildActiveRouteShapesGeoJSON(tables, (source) =>
			source === 'mta_subway' ? subway : source === 'mta_bus' ? bus : undefined
		);
		expect(collection.features).toHaveLength(2);
		const routeA = collection.features.find((f) => f.properties?.id === 'A')!;
		expect(routeA.properties).toMatchObject({
			source: 'mta_subway',
			color: '#0039A6',
			shuttle: false
		});
		expect(routeA.geometry.coordinates).toHaveLength(3);

		const routeM42 = collection.features.find((f) => f.properties?.id === 'M42')!;
		expect(routeM42.properties?.shuttle).toBe(true);
	});

	it('does not overdraw a shared route shape for multiple trips', () => {
		const table = renderUnitTableFromIPC(trajectoryFixture(['v1'], 200));
		const first = table.trips.get('mta_subway:trip-1')!;
		table.trips.set('mta_subway:trip-2', { ...first });
		table.units.push({ ...table.units[0], tripId: 'trip-2' });
		const result = buildActiveRouteShapesGeoJSON(new Map([['mta_subway', table]]), () => subway);
		expect(result.features.map((feature) => feature.geometry.coordinates)).toEqual([
			first.shape.positions
		]);
	});

	it('returns an empty collection when trajectory tables are absent', () => {
		expect(buildActiveRouteShapesGeoJSON(new Map(), () => subway).features).toEqual([]);
	});
});
