import { chartFixture } from '#lib/charts/fixtures.js';
import type { EgressPoint, PlatformEdge } from '#lib/client/index.js';

import { expect, test } from 'vitest';

import { arrivalExits, arrivalTransferExits } from './platform-exits.js';

function point(position_ft: number): EgressPoint {
	return {
		id: position_ft,
		position_ft,
		egress_type: 'STAIRCASE',
		vertical_direction: 'UP',
		street_exits: [{ id: 0, intersection: ['Utica Av'], accessible: false }]
	};
}

function fixture(direction: 1 | 3 = 1) {
	const { stops, trips } = chartFixture('mta_subway');
	const stop = stops[0];
	const trip = trips[0];
	if (stop.data.source !== 'mta_subway' || trip.data.source !== 'mta_subway')
		throw new Error('Expected subway');
	trip.direction = direction;
	trip.data.consist = { car_count: 8, car_length_feet: 60 };
	const edge: PlatformEdge = {
		id: 'edge',
		length_ft: 600,
		car_markers: [
			{
				direction: 'NORTH',
				position_ft: 60,
				consist_length_ft: 480,
				marked_as: '8',
				is_opto: false
			},
			{
				direction: 'SOUTH',
				position_ft: 540,
				consist_length_ft: 480,
				marked_as: '8',
				is_opto: false
			}
		],
		egress_points: [point(90), point(510)]
	};
	stop.data.platform_edges = [edge];
	return { stop, trip, edge };
}

for (const direction of [1, 3] as const) {
	test(`named exits count cars from the front in direction ${direction}`, () => {
		const { stop, trip } = fixture(direction);
		const result = arrivalExits(stop, trip, ['edge'])!;
		expect(result.car_count).toBe(8);
		expect(result.exits.map((exit) => exit.car)).toEqual([8, 1]);
		expect(result.exits.map((exit) => exit.position)).toEqual([0.0625, 0.9375]);
		expect(result.exits.every((exit) => exit.name === 'Utica Av')).toBe(true);
	});
}

test('short trains use their own stopping board and clamp exits beyond the train to its ends', () => {
	const { stop, trip, edge } = fixture();
	if (trip.data.source !== 'mta_subway') throw new Error('Expected subway');
	trip.data.consist = { car_count: 4, car_length_feet: 60 };
	edge.car_markers.push({
		direction: 'NORTH',
		position_ft: 180,
		consist_length_ft: 240,
		marked_as: '4',
		is_opto: false
	});
	expect(
		arrivalExits(stop, trip, ['edge'])?.exits.map((exit) => [exit.car, exit.position])
	).toEqual([
		[4, 0],
		[1, 1]
	]);
});

test('unknown, unmatched and multiple platform assignments render nothing', () => {
	const { stop, trip } = fixture();
	for (const ids of [[], ['missing'], ['edge', 'missing']])
		expect(arrivalExits(stop, trip, ids)).toBeNull();
	expect(arrivalExits(stop, trip, ['edge', 'edge'])).not.toBeNull();
});

test('missing consists, wrong length boards and conflicting stopping positions render nothing', () => {
	const { stop, trip, edge } = fixture();
	if (trip.data.source !== 'mta_subway') throw new Error('Expected subway');
	const consist = trip.data.consist;
	trip.data.consist = null;
	expect(arrivalExits(stop, trip, ['edge'])).toBeNull();
	trip.data.consist = consist;
	edge.car_markers.push({ ...edge.car_markers[0], is_opto: true, position_ft: 100 });
	expect(arrivalExits(stop, trip, ['edge'])).toBeNull();
	edge.car_markers.pop();
	edge.car_markers[0].consist_length_ft = 600;
	expect(arrivalExits(stop, trip, ['edge'])).toBeNull();
});

test('an unnumbered S board works for the supplied train length even with platform overhang', () => {
	const { stop, trip, edge } = fixture();
	edge.car_markers = [{ marked_as: 'S', is_opto: false, direction: 'NORTH', position_ft: 200 }];
	expect(arrivalExits(stop, trip, ['edge'])?.exits.map((exit) => exit.car)).toEqual([6, 1]);
});

test('preserves intersections and path accessibility and excludes unnamed or invalid exits', () => {
	const { stop, trip, edge } = fixture();
	edge.egress_points[0].street_exits = [
		{ id: 7, intersection: ['Utica Av', 'Fulton St'], accessible: true }
	];
	edge.egress_points[1].street_exits = [];
	edge.egress_points.push(point(-1), point(601), point(NaN));
	expect(arrivalExits(stop, trip, ['edge'])?.exits).toEqual([
		expect.objectContaining({ name: 'Utica Av & Fulton St', accessible: true, car: 1 })
	]);
	edge.egress_points[0].street_exits = [];
	expect(arrivalExits(stop, trip, ['edge'])).toBeNull();
	edge.length_ft = 0;
	expect(arrivalExits(stop, trip, ['edge'])).toBeNull();
});

test('unsupported sources have no exit guidance', () => {
	const { stop } = fixture();
	for (const source of ['mta_bus', 'njt_bus'] as const) {
		const bus = chartFixture(source);
		expect(arrivalExits(bus.stops[0], bus.trips[0], [])).toBeNull();
		expect(arrivalExits(stop, bus.trips[0], ['edge'])).toBeNull();
	}
});

test('Borough Hall combines equivalent destinations at one position and keeps separate elevator locations', () => {
	const { stop, trip, edge } = fixture();
	if (trip.data.source !== 'mta_subway') throw new Error('Expected subway');
	trip.data.consist = { car_count: 10, car_length_feet: 51 };
	edge.length_ft = 531;
	edge.car_markers = [
		{ marked_as: '10', is_opto: false, consist_length_ft: 510, position_ft: 0, direction: 'NORTH' }
	];
	edge.egress_points = [
		{
			...point(531),
			id: 1393,
			street_exits: [
				{
					id: 547,
					intersection: ['Joralemon St', 'Municipal Building Brooklyn'],
					accessible: false
				},
				{ id: 2202, intersection: ['Joralemon St', 'Borough Hall'], accessible: false },
				{ id: 2203, intersection: ['Joralemon St', 'Borough Hall'], accessible: true }
			]
		},
		{
			...point(167),
			street_exits: [
				{ id: 1652, intersection: ['Montague St', 'Court St - Cadman Plaza'], accessible: false }
			]
		},
		{
			...point(79),
			egress_type: 'ELEVATOR',
			street_exits: [
				{ id: 1652, intersection: ['Montague St', 'Court St - Cadman Plaza'], accessible: true }
			]
		}
	];
	const exits = arrivalExits(stop, trip, ['edge'])!.exits;
	expect(exits.filter((exit) => exit.name === 'Joralemon St & Borough Hall')).toEqual([
		expect.objectContaining({ accessible: true, car: 10, position: 0 })
	]);
	expect(exits.filter((exit) => exit.name === 'Montague St & Court St - Cadman Plaza')).toEqual([
		expect.objectContaining({ accessible: false, car: 4 }),
		expect.objectContaining({ accessible: true, car: 2 })
	]);
	expect(
		exits.filter((exit) => exit.name === 'Joralemon St & Municipal Building Brooklyn')
	).toHaveLength(1);
});

function transferFixture(direction: 1 | 3 = 1) {
	const f = fixture(direction);
	const target = { ...f.stop, id: 'connecting-stop', data: { ...f.stop.data } };
	if (target.data.source !== 'mta_subway') throw new Error('Expected subway');
	target.data.platform_edges = [{ ...f.edge, id: 'connecting-platform', egress_points: [] }];
	f.edge.egress_points[0].street_exits = [];
	f.edge.egress_points[0].transfer_paths = [
		{ to_platform_edge: 'connecting-platform', accessible: true }
	];
	f.edge.egress_points[1].transfer_paths = [
		{ to_platform_edge: 'other-platform', accessible: false }
	];
	const boarding = { stop: target, distance_m: 20, same_stop: false };
	const departure: import('#lib/transit/nearby.js').DepartureOption = {
		key: 'connecting',
		route: { ...chartFixture('mta_subway').route, id: '7', short_name: '7' },
		direction: 1,
		destination: 'Flushing',
		stops: [boarding],
		unknown_stops: [],
		boarding,
		call: {
			trip_id: 'connecting-trip',
			route_id: '7',
			stop_id: target.id,
			sequence: 4,
			arrival: 1,
			departure: 2,
			platform_edges: ['connecting-platform']
		},
		loading: false,
		error: null
	};
	return { ...f, departure };
}

for (const direction of [1, 3] as const) {
	test(`transfer guidance matches the departure platform across station records in direction ${direction}`, () => {
		const { stop, trip, departure } = transferFixture(direction);
		expect(arrivalTransferExits(stop, trip, ['edge'], departure)).toEqual({
			car_count: 8,
			exits: [
				{
					id: 'transfer:90',
					name: 'Stairs',
					accessible: true,
					car: direction === 1 ? 1 : 8,
					position: direction === 1 ? 0.9375 : 0.0625
				}
			]
		});
	});
}

test('transfer guidance does not infer platforms from route membership or an ambiguous assignment', () => {
	const { stop, trip, edge, departure } = transferFixture();
	for (const ids of [[], ['unknown'], ['connecting-platform', 'other-platform']]) {
		departure.call!.platform_edges = ids;
		expect(arrivalTransferExits(stop, trip, ['edge'], departure)).toBeNull();
	}
	departure.call!.platform_edges = ['connecting-platform', 'connecting-platform'];
	expect(arrivalTransferExits(stop, trip, ['edge'], departure)?.exits).toHaveLength(1);
	expect(arrivalTransferExits(stop, trip, ['edge', 'unknown'], departure)).toBeNull();
	edge.egress_points[0].transfer_paths = [];
	expect(arrivalTransferExits(stop, trip, ['edge'], departure)).toBeNull();
});

test('transfer guidance retains distinct stairs and elevator paths and their own accessibility', () => {
	const { stop, trip, edge, departure } = transferFixture();
	edge.egress_points[1].egress_type = 'ELEVATOR';
	edge.egress_points[1].transfer_paths = [
		{ to_platform_edge: 'connecting-platform', accessible: false }
	];
	expect(arrivalTransferExits(stop, trip, ['edge'], departure)?.exits).toEqual([
		{ id: 'transfer:510', name: 'Elevator', accessible: false, car: 8, position: 0.0625 },
		{ id: 'transfer:90', name: 'Stairs', accessible: true, car: 1, position: 0.9375 }
	]);
	edge.car_markers = [];
	expect(arrivalTransferExits(stop, trip, ['edge'], departure)).toBeNull();
});
