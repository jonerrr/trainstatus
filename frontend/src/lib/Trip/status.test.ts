import type { CatalogStop, ServiceTrip } from '#lib/client/index.js';
import type { Call } from '#lib/transit/decode.js';

import { describe, expect, it } from 'vitest';

import { trip_status_summary, trip_vehicle_summary } from './status.js';

describe('trip_status_summary', () => {
	const now = 1000000;
	const stops: [string, CatalogStop][] = [
		[
			'stop-1',
			{
				id: 'stop-1',
				name: 'Times Sq-42 St',
				lat: 40.75,
				lon: -73.98,
				nearby_stops: [],
				routes: [],
				data: { source: 'mta_subway', station_group_id: '1' } as never
			}
		],
		[
			'stop-2',
			{
				id: 'stop-2',
				name: '34 St-Penn Station',
				lat: 40.75,
				lon: -73.99,
				nearby_stops: [],
				routes: [],
				data: { source: 'mta_subway', station_group_id: '2' } as never
			}
		]
	];
	const stops_map = new Map(stops);

	const call1: Call = {
		trip_id: 'trip-1',
		route_id: '1',
		stop_id: 'stop-1',
		sequence: 1,
		arrival: now + 120_000, // 2 minutes from now
		departure: now + 120_000,
		platform_edges: []
	};

	const call2: Call = {
		trip_id: 'trip-1',
		route_id: '1',
		stop_id: 'stop-2',
		sequence: 2,
		arrival: now + 360_000, // 6 minutes from now
		departure: now + 360_000,
		platform_edges: []
	};

	it('formats EN_ROUTE as en route with target stop and eta', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			status: {
				updated_at: now / 1000,
				stop_id: 'stop-1',
				data: { source: 'mta_subway', assigned: true, status: 'EN_ROUTE' }
			},
			data: { source: 'mta_subway', consist_cars: [] }
		};

		const summary = trip_status_summary(trip, [call1, call2], stops_map, now);
		expect(summary.description).toBe('En route to Times Sq-42 St');
		expect(summary.stop_name).toBe('Times Sq-42 St');
		expect(summary.eta_label).toBe('2m');
		expect(summary.eta_minutes).toBe(2);
		expect(summary.stops_remaining).toBe(2);
	});

	it('formats AT_STOP as at stop', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			status: {
				updated_at: now / 1000,
				stop_id: 'stop-1',
				data: { source: 'mta_subway', assigned: true, status: 'AT_STOP' }
			},
			data: { source: 'mta_subway', consist_cars: [] }
		};

		const summary = trip_status_summary(trip, [call1, call2], stops_map, now);
		expect(summary.description).toBe('At Times Sq-42 St');
		expect(summary.eta_label).toBe('2m');
		expect(summary.eta_minutes).toBe(2);
	});

	it.each([
		['AT_STOP', 'At Times Sq-42 St', 'Now'],
		['EN_ROUTE', 'En route to Times Sq-42 St', 'Due']
	])(
		'keeps %s countdown at the observed stop after its prediction passes',
		(status, description, eta_label) => {
			const trip: ServiceTrip = {
				id: 'trip-1',
				vehicle_id: 'train-1',
				direction: 1,
				status: {
					updated_at: now / 1000,
					stop_id: 'stop-1',
					data: { source: 'mta_subway', assigned: true, status }
				},
				data: { source: 'mta_subway', consist_cars: [] }
			};
			const past_call = { ...call1, arrival: now - 15_000, departure: now - 15_000 };

			const summary = trip_status_summary(trip, [past_call, call2], stops_map, now);
			expect(summary.description).toBe(description);
			expect(summary.eta_label).toBe(eta_label);
			expect(summary.eta_minutes).toBe(0);
		}
	);

	it('uses the displayed stop prediction even when another stop is due sooner', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			status: {
				updated_at: now / 1000,
				stop_id: 'stop-2',
				data: { source: 'mta_subway', assigned: true, status: 'EN_ROUTE' }
			},
			data: { source: 'mta_subway', consist_cars: [] }
		};

		const summary = trip_status_summary(trip, [call1, call2], stops_map, now);
		expect(summary.description).toBe('En route to 34 St-Penn Station');
		expect(summary.eta_label).toBe('6m');
	});

	it('omits the countdown when the observed stop has no matching call', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			status: {
				updated_at: now / 1000,
				stop_id: 'stop-2',
				data: { source: 'mta_subway', assigned: true, status: 'EN_ROUTE' }
			},
			data: { source: 'mta_subway', consist_cars: [] }
		};

		const summary = trip_status_summary(trip, [call1], stops_map, now);
		expect(summary.description).toBe('En route to 34 St-Penn Station');
		expect(summary.eta_label).toBeNull();
		expect(summary.eta_minutes).toBeNull();
	});

	it('formats INCOMING_AT / approaching as Approaching', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'bus-1',
			direction: 0,
			status: {
				updated_at: now / 1000,
				stop_id: 'stop-1',
				data: { source: 'mta_bus', status: 'approaching' }
			},
			data: { source: 'mta_bus' }
		};

		const summary = trip_status_summary(trip, [call1], stops_map, now);
		expect(summary.description).toBe('Approaching Times Sq-42 St');
		expect(summary.stops_remaining).toBe(1);
	});

	it('formats unassigned subway train as departs from origin', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			status: {
				updated_at: now / 1000,
				stop_id: null,
				data: { source: 'mta_subway', assigned: false, status: 'SCHEDULED' }
			},
			data: { source: 'mta_subway', consist_cars: [] }
		};

		const summary = trip_status_summary(trip, [call1, call2], stops_map, now);
		expect(summary.description).toBe('Departs from Times Sq-42 St');
	});

	it('handles null status by using next call stop', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'bus-1',
			direction: 0,
			status: null,
			data: { source: 'njt_bus', headsign: 'New York' }
		};

		const summary = trip_status_summary(trip, [call1, call2], stops_map, now);
		expect(summary.description).toBe('Approaching Times Sq-42 St');
		expect(summary.eta_label).toBe('2m');
	});

	it('handles completed trip when all calls have passed', () => {
		const past_call: Call = {
			...call1,
			arrival: now - 60_000,
			departure: now - 30_000
		};
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			status: null,
			data: { source: 'mta_subway', consist_cars: [] }
		};

		const summary = trip_status_summary(trip, [past_call], stops_map, now);
		expect(summary.description).toBe('Trip completed');
		expect(summary.stops_remaining).toBe(0);
		expect(summary.eta_label).toBeNull();
		expect(summary.eta_minutes).toBeNull();
	});

	it('shows Due when arrival is within 30 seconds', () => {
		const due_call: Call = {
			...call1,
			arrival: now + 15_000
		};
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			status: null,
			data: { source: 'mta_subway', consist_cars: [] }
		};

		const summary = trip_status_summary(trip, [due_call], stops_map, now);
		expect(summary.eta_label).toBe('Due');
		expect(summary.eta_minutes).toBe(0);
	});

	it('handles dwell/layover when arrival is in the past but departure is in the future', () => {
		const dwell_call: Call = {
			...call1,
			arrival: now - 60_000,
			departure: now + 120_000 // 2 minutes from now
		};
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			status: {
				updated_at: now / 1000,
				stop_id: 'stop-1',
				data: { source: 'mta_subway', assigned: true, status: 'EN_ROUTE' }
			},
			data: { source: 'mta_subway', consist_cars: [] }
		};

		const summary = trip_status_summary(trip, [dwell_call, call2], stops_map, now);
		expect(summary.eta_minutes).toBe(2);
		expect(summary.eta_label).toBe('2m');
		expect(summary.stops_remaining).toBe(2);
	});

	it('shows Due during dwell/layover when departure is within 30 seconds', () => {
		const dwell_due_call: Call = {
			...call1,
			arrival: now - 60_000,
			departure: now + 20_000 // 20 seconds from now
		};
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			status: null,
			data: { source: 'mta_subway', consist_cars: [] }
		};

		const summary = trip_status_summary(trip, [dwell_due_call], stops_map, now);
		expect(summary.eta_minutes).toBe(0);
		expect(summary.eta_label).toBe('Due');
	});
});

describe('trip_vehicle_summary', () => {
	it('formats subway consist model and car count', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			data: {
				source: 'mta_subway',
				consist: { car_count: 10, car_length_feet: 60 },
				consist_cars: [{ number: '7244', type: 'R142A' }]
			}
		};

		const summary = trip_vehicle_summary(trip);
		expect(summary.vehicle_label).toBe('R142A · 10 cars');
	});

	it('formats subway consist with car count only', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			data: {
				source: 'mta_subway',
				consist: { car_count: 8, car_length_feet: 60 },
				consist_cars: []
			}
		};

		const summary = trip_vehicle_summary(trip);
		expect(summary.vehicle_label).toBe('8 cars');
	});

	it('formats bus vehicle ID and delay deviation', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: '5842',
			direction: 0,
			data: {
				source: 'mta_bus',
				deviation: 240 // 4 mins late
			}
		};

		const summary = trip_vehicle_summary(trip);
		expect(summary.vehicle_label).toBe('Bus #5842');
		expect(summary.deviation_text).toBe('4 min late');
		expect(summary.deviation_tone).toBe('warning');
	});

	it('formats early bus deviation', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: '5842',
			direction: 0,
			data: {
				source: 'mta_bus',
				deviation: -180 // 3 mins early
			}
		};

		const summary = trip_vehicle_summary(trip);
		expect(summary.deviation_text).toBe('3 min early');
		expect(summary.deviation_tone).toBe('secondary');
	});

	it('omits deviation if within 2 minutes', () => {
		const trip: ServiceTrip = {
			id: 'trip-1',
			vehicle_id: '5842',
			direction: 0,
			data: {
				source: 'mta_bus',
				deviation: 60 // 1 min (not significant)
			}
		};

		const summary = trip_vehicle_summary(trip);
		expect(summary.deviation_text).toBeUndefined();
	});

	it('returns safe undefined summary for unexpected source', () => {
		const trip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			data: { source: 'unexpected_source' as never }
		} as ServiceTrip;

		const summary = trip_vehicle_summary(trip);
		expect(summary).toEqual({
			vehicle_label: undefined,
			deviation_text: undefined,
			deviation_tone: undefined
		});
	});
});
