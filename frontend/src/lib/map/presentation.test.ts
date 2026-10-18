import type { CatalogRoute, ServiceTrip } from '#lib/client/index.js';
import { trip_destination_title } from '#lib/util.svelte.js';

import { expect, test } from 'vitest';

import { presentFeature } from './presentation';

const route: CatalogRoute = {
	id: 'SBS-M15',
	short_name: 'M15-SBS',
	long_name: 'East Side',
	color: '#006699',
	text_color: '#ffffff',
	data: {
		source: 'mta_bus',
		name_number: 15,
		name_prefix: 'M',
		name_suffix: '-SBS',
		service_types: [],
		sort_key: 15,
		directions: [{ direction_id: 0, headsign: 'South Ferry' }]
	}
};
const trip: ServiceTrip = {
	id: 'internal-trip',
	vehicle_id: '1234',
	direction: 0,
	status: null,
	data: { source: 'mta_bus' }
};
test('vehicle summaries show route and destination rather than internal IDs', () => {
	const result = presentFeature({ kind: 'trip', source: 'mta_bus', id: trip.id }, { route, trip });
	expect(result.title).toBe('South Ferry');
	expect(JSON.stringify(result)).not.toContain('internal-trip');
	expect(result.subtitle).not.toContain('1234');
});
test('unknown headsigns use the same route-name fallback as trip details', () => {
	expect(trip_destination_title('Unknown', route)).toBe('East Side');
	expect(trip_destination_title(undefined, route)).toBe('East Side');
	expect(trip_destination_title('South Ferry', route)).toBe('South Ferry');
});
test('missing trips keep a useful route label without inventing a destination', () => {
	const result = presentFeature({ kind: 'trip', source: 'mta_bus', id: 'expired' }, { route });
	expect(result.title).toBe('East Side');
	expect(result.detail).toBe('Trip details unavailable');
});
