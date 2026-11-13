import type { CatalogStop } from '#lib/client/index.js';

import { describe, expect, it } from 'vitest';

import { decodeCatalog } from './data/decode';
import { searchStops } from './search';

function stop(id: string, name: string): CatalogStop {
	return {
		id,
		name,
		lat: 0,
		lon: 0,
		routes: [],
		nearby_stops: [],
		data: {
			source: 'mta_subway',
			bubble_id: id,
			gtfs_stop_id: id,
			is_major: false,
			line: '',
			north_headsign: '',
			platform_edges: [],
			south_headsign: '',
			station_group_id: id
		}
	};
}

const timesSquare = stop('127', 'Times Sq-42 St');
const astoria = stop('R01', 'Astoria-Ditmars Blvd');

describe('searchStops', () => {
	const catalog = decodeCatalog({
		source: 'mta_subway',
		routes: [],
		stops: [timesSquare, astoria]
	});

	it('returns the stop whose name matches', () => {
		expect(searchStops(catalog, 'Ditmars')).toEqual([astoria]);
	});

	it('returns every stop when the term is empty', () => {
		expect(searchStops(catalog, '   ')).toEqual([timesSquare, astoria]);
	});

	it('indexes a replacement catalog independently', () => {
		const replacement = decodeCatalog({ source: 'mta_subway', routes: [], stops: [timesSquare] });
		expect(searchStops(replacement, 'Ditmars')).toEqual([]);
		expect(searchStops(catalog, 'Ditmars')).toEqual([astoria]);
	});
});
