import { describe, expect, it } from 'vitest';

import type { Route } from './client';
import { bus_headsign } from './util.svelte';

/**
 * The Helium feed maps headsigns to a route's directions, never to individual
 * stops, so `RouteStop.data.headsign` is always empty for MTA buses and every
 * bus headsign in the UI has to be resolved from the route plus the trip's
 * direction. These cover the shapes the feed actually produces.
 */

function bus_route(
	directions: { direction_id: number; destination: string; via: string[] }[] | undefined
): Route {
	return {
		id: 'B1',
		short_name: 'B1',
		long_name: 'Manhattan Beach Kingsboro CC - Bay Ridge 4 Av',
		color: '#00AEEF',
		text_color: '#FFFFFF',
		data: {
			source: 'mta_bus',
			sort_key: 0,
			service_types: ['LOCAL'],
			borough: 'brooklyn',
			name_prefix: 'B',
			name_number: 1,
			name_suffix: null,
			shape_ids: [],
			directions
		}
	} as unknown as Route;
}

describe('bus_headsign', () => {
	it('resolves the destination for each direction', () => {
		const route = bus_route([
			{ direction_id: 0, destination: 'Bay Ridge 4 Av', via: [] },
			{ direction_id: 1, destination: 'Manhattan Beach Kingsboro CC', via: [] }
		]);

		expect(bus_headsign(route, 0)).toBe('Bay Ridge 4 Av');
		expect(bus_headsign(route, 1)).toBe('Manhattan Beach Kingsboro CC');
	});

	it('appends the via streets the MTA prints under the destination', () => {
		const route = bus_route([
			{ direction_id: 0, destination: 'Bergen Beach E 71 St', via: ['Av U'] },
			{ direction_id: 1, destination: 'Bed-Stuy Bway-Halsey', via: ['Kings Hwy', 'Utica'] }
		]);

		expect(bus_headsign(route, 0)).toBe('Bergen Beach E 71 St via Av U');
		expect(bus_headsign(route, 1)).toBe('Bed-Stuy Bway-Halsey via Kings Hwy/Utica');
	});

	it('returns undefined for a direction the route does not publish', () => {
		// Seven routes (B74, S81, Q70+, ...) only publish one direction, and the
		// one they publish is not always direction 0.
		const route = bus_route([{ direction_id: 1, destination: 'Stillwell Av', via: [] }]);

		expect(bus_headsign(route, 1)).toBe('Stillwell Av');
		expect(bus_headsign(route, 0)).toBeUndefined();
	});

	it('returns undefined for a missing route or a non-bus route', () => {
		expect(bus_headsign(undefined, 0)).toBeUndefined();
		expect(
			bus_headsign({ id: '1', data: { source: 'mta_subway' } } as unknown as Route, 0)
		).toBeUndefined();
	});

	it('tolerates directions being absent, since the schema marks it optional', () => {
		expect(bus_headsign(bus_route(undefined), 0)).toBeUndefined();
	});
});
