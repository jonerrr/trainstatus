import type { Stop } from '#lib/client/index.js';

import { describe, expect, it } from 'vitest';

import { StopSearch } from './search.svelte';

function stop(id: string, name: string): Stop {
	return {
		id,
		name,
		geom: { Point: { x: 0, y: 0 } },
		routes: [],
		transfers: [],
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

describe('StopSearch.query', () => {
	const search = new StopSearch({ mta_subway: [timesSquare, astoria] });

	it('returns the stop whose name matches', () => {
		expect(search.query('Ditmars', 'mta_subway')).toEqual([astoria]);
	});

	it('returns every stop for that source when the term is empty', () => {
		expect(search.query('   ', 'mta_subway')).toEqual([timesSquare, astoria]);
	});

	it('returns nothing for a source that was not indexed', () => {
		expect(search.query('Ditmars', 'njt_bus')).toEqual([]);
	});
});
