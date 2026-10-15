import { describe, expect, it, vi } from 'vitest';

import type { CatalogRoute, CatalogStopRoute } from './client';
import { bus_headsign, debounce, main_route_stops, throttle } from './util.svelte';

it('throttles continuous input to the latest value each interval and cancels pending work', () => {
	vi.useFakeTimers();
	try {
		const search = vi.fn<(term: string) => void>();
		const throttled = throttle(search, 150);
		throttled('a');
		expect(search.mock.calls).toEqual([['a']]);
		vi.advanceTimersByTime(50);
		throttled('ab');
		vi.advanceTimersByTime(50);
		throttled('abc');
		vi.advanceTimersByTime(49);
		expect(search.mock.calls).toEqual([['a']]);
		vi.advanceTimersByTime(1);
		expect(search.mock.calls).toEqual([['a'], ['abc']]);
		throttled('abcd');
		vi.advanceTimersByTime(150);
		expect(search.mock.calls).toEqual([['a'], ['abc'], ['abcd']]);
		throttled('');
		throttled.cancel();
		vi.runAllTimers();
		expect(search.mock.calls).toEqual([['a'], ['abc'], ['abcd']]);
		vi.advanceTimersByTime(150);
		throttled('new');
		expect(search.mock.calls.at(-1)).toEqual(['new']);
	} finally {
		vi.useRealTimers();
	}
});

it('debounces rapid calls to the latest target and cancels pending work', () => {
	vi.useFakeTimers();
	try {
		const scroll = vi.fn<(target: number) => void>();
		const debounced = debounce(scroll);
		debounced(1);
		vi.advanceTimersByTime(50);
		debounced(2);
		vi.advanceTimersByTime(75);
		expect(scroll.mock.calls).toEqual([[2]]);
		debounced(3);
		debounced.cancel();
		vi.runAllTimers();
		expect(scroll.mock.calls).toEqual([[2]]);
	} finally {
		vi.useRealTimers();
	}
});

describe('main_route_stops', () => {
	const routes: CatalogStopRoute[] = ['6', '6X', '7', '7X', 'FX'].map((route_id) => ({
		route_id,
		stop_sequence: 1,
		data: { source: 'mta_subway', stop_type: 'full_time' }
	}));

	it('omits express variants without trips while keeping ordinary routes', () => {
		expect(main_route_stops(routes, new Set()).map((r) => r.route_id)).toEqual(['6', '7']);
	});

	it('shows only express variants with actual trips, then hides them when service clears', () => {
		const active = new Set(['7X', 'FX']);
		expect(main_route_stops(routes, active).map((r) => r.route_id)).toEqual(['6', '7', '7X', 'FX']);
		active.clear();
		expect(main_route_stops(routes, active).map((r) => r.route_id)).toEqual(['6', '7']);
	});
});

/**
 * The Helium feed maps headsigns to a route's directions, never to individual
 * stops, so `CatalogStopRoute.data.headsign` is always empty for MTA buses and every
 * bus headsign in the UI has to be resolved from the route plus the trip's
 * direction. These cover the shapes the feed actually produces.
 */

function bus_route(
	directions: { direction_id: number; destination: string; via: string[] }[] | undefined
): CatalogRoute {
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
			directions
		}
	} as unknown as CatalogRoute;
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
			bus_headsign({ id: '1', data: { source: 'mta_subway' } } as unknown as CatalogRoute, 0)
		).toBeUndefined();
	});

	it('tolerates directions being absent, since the schema marks it optional', () => {
		expect(bus_headsign(bus_route(undefined), 0)).toBeUndefined();
	});
});
