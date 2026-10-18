import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import { nearbyFixture } from '#lib/test/nearby-fixture.js';

import { describe, expect, it } from 'vitest';

import { decodeCatalog, decodeRoute } from './decode.js';
import {
	boardingDistance,
	boardingOptions,
	compareDepartures,
	departureOptions,
	optionStops,
	serviceKey
} from './nearby.js';
import type { RouteRead } from './store.svelte.js';

function setup() {
	const fixture = nearbyFixture();
	const catalogs = [decodeCatalog(fixture.catalog)];
	const reads: RouteRead[] = Object.entries(fixture.service.routes).map(([route_id, service]) => ({
		source: 'mta_bus',
		route_id,
		route: decodeRoute('mta_bus', route_id, service),
		loading: false,
		error: null
	}));
	return { ...fixture, catalogs, reads };
}

describe('nearby boarding choices', () => {
	it('deduplicates each service without dropping the farther stop’s unique route', () => {
		const f = setup();
		const options = boardingOptions(f.origin, f.catalogs);
		expect(options.map((o) => [o.route.id, o.stops[0].stop.id])).toEqual([
			['A', 'near'],
			['B', 'far'],
			['C', 'third'],
			['D', 'fourth']
		]);
		expect(options[0].stops.map((s) => s.stop.id)).toEqual(['near', 'far']);
	});
	it('suppresses only the exact known service at the origin and preserves the opposite direction', () => {
		const f = setup();
		f.origin.routes.push({
			route_id: 'A',
			stop_sequence: 0,
			data: { source: 'mta_bus', direction_ids: [0] }
		});
		f.near.routes[0].data = { source: 'mta_bus', direction_ids: [0, 1] };
		const options = boardingOptions(f.origin, f.catalogs);
		expect(options.filter((o) => o.route.id === 'A').map((o) => o.direction)).toEqual([1]);
		expect(options.some((o) => o.route.id === 'B')).toBe(true);
	});
	it('keeps source-scoped route and stop IDs distinct, including bus to subway choices', () => {
		const f = setup();
		const subway = chartFixture('mta_subway');
		subway.route.id = 'A';
		subway.stops[0].id = 'near';
		subway.stops[0].routes[0].route_id = 'A';
		f.origin.nearby_stops.push({ source: 'mta_subway', stop_id: 'near', distance_m: 25 });
		const options = boardingOptions(f.origin, [...f.catalogs, decodeCatalog(subway.catalog)]);
		expect(
			options.filter((o) => o.route.id === 'A').map((o) => [o.route.data.source, o.direction])
		).toEqual([
			['mta_bus', 0],
			['mta_subway', 1],
			['mta_subway', 3]
		]);
		expect(
			boardingOptions(f.origin, [...f.catalogs, decodeCatalog(subway.catalog)], {
				...f.incoming,
				route_id: 'A'
			})
				.filter((o) => o.route.id === 'A')
				.map((o) => [o.route.data.source, o.direction])
		).toEqual([
			['mta_subway', 1],
			['mta_subway', 3]
		]);
	});
	it('preserves unknown alternatives without inferring their direction or duplicating known badges', () => {
		const f = setup();
		f.far.routes[0].data = { source: 'mta_bus', direction_ids: [] };
		f.far.routes[1].data = { source: 'mta_bus', direction_ids: [] };
		const options = boardingOptions(f.origin, f.catalogs);
		const a = options.find((o) => o.route.id === 'A')!;
		expect(a.direction).toBe(0);
		expect(a.unknown_stops.map((s) => s.stop.id)).toEqual(['far']);
		expect(optionStops(a).map((s) => s.stop.id)).toEqual(['near', 'far']);
		expect(options.find((o) => o.route.id === 'B')?.direction).toBeNull();
	});
	it('resolves equal-distance stops deterministically regardless of candidate order', () => {
		const f = setup();
		f.origin.nearby_stops[1].distance_m = 20;
		const before = boardingOptions(f.origin, f.catalogs)[0].stops.map((s) => s.stop.id);
		f.origin.nearby_stops.reverse();
		expect(boardingOptions(f.origin, f.catalogs)[0].stops.map((s) => s.stop.id)).toEqual(before);
	});
});

describe('nearby departures', () => {
	it('uses departure time across every boarding alternative, then ranks before truncating', () => {
		const f = setup();
		const options = boardingOptions(f.origin, f.catalogs, f.incoming);
		const departures = departureOptions(options, f.reads, f.incoming, f.catalogs, chartNow);
		const a = departures.find((o) => o.route.id === 'A')!;
		expect(a.boarding.stop.id).toBe('far');
		expect(a.call?.departure).toBe(chartNow + 5 * 60_000);
		expect(a.next?.boarding.stop.id).toBe('near');
		expect([...departures].sort(compareDepartures).map((o) => o.route.id)).toEqual([
			'D',
			'A',
			'C',
			'B'
		]);
	});
	it('restricts initial results to requested routes and retains a static fallback for empty predictions', () => {
		const f = setup();
		const options = boardingOptions(f.origin, f.catalogs, f.incoming).slice(0, 3);
		const result = departureOptions(options, f.reads, f.incoming, f.catalogs, chartNow);
		expect(result.map((o) => o.route.id)).toEqual(['A', 'B', 'C']);
		const empty = f.reads.map((r) => ({
			...r,
			route: r.route && { ...r.route, calls_by_stop: new Map() }
		}));
		const fallback = departureOptions(options, empty, f.incoming, f.catalogs, chartNow);
		expect(fallback.every((o) => !o.call && !o.loading && !o.error)).toBe(true);
		expect(fallback[0].destination).toBe('A North');
	});
	it('excludes the current route in every direction, including unknown memberships', () => {
		const f = setup();
		f.incoming = { ...f.incoming, route_id: 'A' };
		f.near.routes[0].data = { source: 'mta_bus', direction_ids: [0, 1] };
		f.far.routes[0].data = { source: 'mta_bus', direction_ids: [] };
		expect(boardingOptions(f.origin, f.catalogs, f.incoming).some((o) => o.route.id === 'A')).toBe(
			false
		);
		const departures = departureOptions(
			boardingOptions(f.origin, f.catalogs),
			f.reads,
			f.incoming,
			f.catalogs,
			chartNow
		);
		expect(departures.some((o) => o.route.id === 'A')).toBe(false);
		expect(departures.some((o) => o.route.id === 'B' && o.call)).toBe(true);
	});
	it('lets a real call establish an unknown stop’s direction without rewriting its catalog', () => {
		const f = setup();
		f.near.routes[0].data = { source: 'mta_bus', direction_ids: [] };
		f.far.routes[0].data = { source: 'mta_bus', direction_ids: [] };
		const read = f.reads.find((r) => r.route_id === 'A')!;
		read.route = decodeRoute(
			'mta_bus',
			'A',
			f.serviceFor('A', [
				['near', 4, 1],
				['far', 5, 0]
			])
		);
		const result = departureOptions(
			boardingOptions(f.origin, f.catalogs, f.incoming),
			f.reads,
			f.incoming,
			f.catalogs,
			chartNow
		);
		expect(
			result
				.filter((o) => o.route.id === 'A')
				.map((o) => o.key)
				.sort()
		).toEqual([serviceKey('mta_bus', 'A', 0), serviceKey('mta_bus', 'A', 1)].sort());
		expect(f.near.routes[0].data.direction_ids).toEqual([]);
	});
	it('retains dwell departures after arrival, and uses the supplied historical reference rather than the wall clock', () => {
		const f = setup();
		const read = f.reads.find((r) => r.route_id === 'A')!;
		const service = f.serviceFor('A', [['near', 2, 0]]);
		service.calls.arrival[0] = chartNow / 1000;
		read.route = decodeRoute('mta_bus', 'A', service);
		const options = boardingOptions(f.origin, f.catalogs, f.incoming);
		expect(
			departureOptions(options, f.reads, f.incoming, f.catalogs, chartNow)[0].call?.departure
		).toBe(chartNow + 120_000);
		expect(
			departureOptions(options, f.reads, f.incoming, f.catalogs, chartNow + 180_000)[0].call
		).toBeUndefined();
	});
});

it('uses stop identity rather than coincident coordinates for the Here label', () => {
	const f = setup();
	f.origin.nearby_stops[0].distance_m = 0;
	const nearby = boardingOptions(f.origin, f.catalogs)[0].stops[0];
	expect(boardingDistance(nearby.distance_m, nearby.same_stop, 'en-CA')).toBe('<10 m');
	f.origin.routes.push(f.near.routes[0]);
	const here = boardingOptions(f.origin, f.catalogs, f.incoming)
		.find((o) => o.route.id === 'A')!
		.stops.find((s) => s.stop.id === f.origin.id)!;
	expect(boardingDistance(here.distance_m, here.same_stop)).toBe('Here');
});

it('formats nearby distances using the locale’s regional units without false precision', () => {
	expect(boardingDistance(40, false, 'en-US')).toBe('~130 ft');
	expect(boardingDistance(40, false, 'es-US')).toContain('130');
	expect(boardingDistance(40, false, 'en-GB')).toBe('~40 yd');
	expect(boardingDistance(40, false, 'en-CA')).toBe('~40 m');
	expect(boardingDistance(40, false, 'fr-FR')).toMatch(/~40\s+m/);
	expect(boardingDistance(1, false, 'en-US')).toBe('<10 ft');
	expect(boardingDistance(1, false, 'en-GB')).toBe('<10 yd');
	expect(boardingDistance(0, true, 'en-US')).toBe('Here');
});

it('retains a known direction fallback while an unknown alternative reveals the opposite direction', () => {
	const f = setup();
	f.far.routes[0].data = { source: 'mta_bus', direction_ids: [] };
	const read = f.reads.find((r) => r.route_id === 'A')!;
	read.route = decodeRoute('mta_bus', 'A', f.serviceFor('A', [['far', 5, 1]]));
	const result = departureOptions(
		boardingOptions(f.origin, f.catalogs, f.incoming),
		f.reads,
		f.incoming,
		f.catalogs,
		chartNow
	).filter((o) => o.route.id === 'A');
	expect(result.map((o) => [o.direction, !!o.call]).sort()).toEqual([
		[0, false],
		[1, true]
	]);
});
