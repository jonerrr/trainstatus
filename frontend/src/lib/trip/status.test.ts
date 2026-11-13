import type { CatalogStop, ServiceTrip } from '#lib/client/index.js';
import type { Call } from '#lib/data/decode.js';

import { describe, expect, it } from 'vitest';

import { tripStatusSummary, tripVehicleSummary } from './status.js';

describe('tripStatusSummary', () => {
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
	const stopsMap = new Map(stops);

	const call1: Call = {
		tripId: 'trip-1',
		routeId: '1',
		stopId: 'stop-1',
		sequence: 1,
		arrival: now + 120_000, // 2 minutes from now
		departure: now + 120_000,
		platformEdges: []
	};

	const call2: Call = {
		tripId: 'trip-1',
		routeId: '1',
		stopId: 'stop-2',
		sequence: 2,
		arrival: now + 360_000, // 6 minutes from now
		departure: now + 360_000,
		platformEdges: []
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

		const summary = tripStatusSummary(trip, [call1, call2], stopsMap, now);
		expect(summary.description).toBe('En route to Times Sq-42 St');
		expect(summary.stopName).toBe('Times Sq-42 St');
		expect(summary.etaLabel).toBe('2m');
		expect(summary.etaMinutes).toBe(2);
		expect(summary.stopsRemaining).toBe(2);
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

		const summary = tripStatusSummary(trip, [call1, call2], stopsMap, now);
		expect(summary.description).toBe('At Times Sq-42 St');
		expect(summary.etaLabel).toBe('2m');
		expect(summary.etaMinutes).toBe(2);
	});

	it.each([
		['AT_STOP', 'At Times Sq-42 St', 'Now'],
		['EN_ROUTE', 'En route to Times Sq-42 St', 'Due']
	])(
		'keeps %s countdown at the observed stop after its prediction passes',
		(status, description, etaLabel) => {
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
			const pastCall = { ...call1, arrival: now - 15_000, departure: now - 15_000 };

			const summary = tripStatusSummary(trip, [pastCall, call2], stopsMap, now);
			expect(summary.description).toBe(description);
			expect(summary.etaLabel).toBe(etaLabel);
			expect(summary.etaMinutes).toBe(0);
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

		const summary = tripStatusSummary(trip, [call1, call2], stopsMap, now);
		expect(summary.description).toBe('En route to 34 St-Penn Station');
		expect(summary.etaLabel).toBe('6m');
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

		const summary = tripStatusSummary(trip, [call1], stopsMap, now);
		expect(summary.description).toBe('En route to 34 St-Penn Station');
		expect(summary.etaLabel).toBeNull();
		expect(summary.etaMinutes).toBeNull();
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

		const summary = tripStatusSummary(trip, [call1], stopsMap, now);
		expect(summary.description).toBe('Approaching Times Sq-42 St');
		expect(summary.stopsRemaining).toBe(1);
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

		const summary = tripStatusSummary(trip, [call1, call2], stopsMap, now);
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

		const summary = tripStatusSummary(trip, [call1, call2], stopsMap, now);
		expect(summary.description).toBe('Approaching Times Sq-42 St');
		expect(summary.etaLabel).toBe('2m');
	});

	it('handles completed trip when all calls have passed', () => {
		const pastCall: Call = {
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

		const summary = tripStatusSummary(trip, [pastCall], stopsMap, now);
		expect(summary.description).toBe('Trip completed');
		expect(summary.stopsRemaining).toBe(0);
		expect(summary.etaLabel).toBeNull();
		expect(summary.etaMinutes).toBeNull();
	});

	it('shows Due when arrival is within 30 seconds', () => {
		const dueCall: Call = {
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

		const summary = tripStatusSummary(trip, [dueCall], stopsMap, now);
		expect(summary.etaLabel).toBe('Due');
		expect(summary.etaMinutes).toBe(0);
	});

	it('handles dwell/layover when arrival is in the past but departure is in the future', () => {
		const dwellCall: Call = {
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

		const summary = tripStatusSummary(trip, [dwellCall, call2], stopsMap, now);
		expect(summary.etaMinutes).toBe(2);
		expect(summary.etaLabel).toBe('2m');
		expect(summary.stopsRemaining).toBe(2);
	});

	it('shows Due during dwell/layover when departure is within 30 seconds', () => {
		const dwellDueCall: Call = {
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

		const summary = tripStatusSummary(trip, [dwellDueCall], stopsMap, now);
		expect(summary.etaMinutes).toBe(0);
		expect(summary.etaLabel).toBe('Due');
	});
});

describe('tripVehicleSummary', () => {
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

		const summary = tripVehicleSummary(trip);
		expect(summary.vehicleLabel).toBe('R142A · 10 cars');
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

		const summary = tripVehicleSummary(trip);
		expect(summary.vehicleLabel).toBe('8 cars');
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

		const summary = tripVehicleSummary(trip);
		expect(summary.vehicleLabel).toBe('Bus #5842');
		expect(summary.deviationText).toBe('4 min late');
		expect(summary.deviationTone).toBe('warning');
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

		const summary = tripVehicleSummary(trip);
		expect(summary.deviationText).toBe('3 min early');
		expect(summary.deviationTone).toBe('secondary');
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

		const summary = tripVehicleSummary(trip);
		expect(summary.deviationText).toBeUndefined();
	});

	it('returns safe undefined summary for unexpected source', () => {
		const trip = {
			id: 'trip-1',
			vehicle_id: 'train-1',
			direction: 1,
			data: { source: 'unexpected_source' as never }
		} as ServiceTrip;

		const summary = tripVehicleSummary(trip);
		expect(summary).toEqual({
			vehicleLabel: undefined,
			deviationText: undefined,
			deviationTone: undefined
		});
	});
});
