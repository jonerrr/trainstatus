import type { CatalogRoute, CatalogStop, ServiceTrip } from '#lib/client/index.js';
import type { Call } from '#lib/transit/decode.js';
import { trip_destination_title, trip_headsign } from '#lib/util.svelte.js';

import type { MapTarget } from './interactions';

export interface FeaturePresentation {
	route?: CatalogRoute;
	title: string;
	subtitle?: string;
	detail?: string;
}

export function presentFeature(
	target: MapTarget,
	{
		route,
		stop,
		trip,
		calls = [],
		stops = new Map(),
		at = Date.now()
	}: {
		route?: CatalogRoute;
		stop?: CatalogStop;
		trip?: ServiceTrip;
		calls?: readonly Call[];
		stops?: ReadonlyMap<string, CatalogStop>;
		at?: number;
	}
): FeaturePresentation {
	const subtitle =
		target.kind === 'trip'
			? target.source === 'mta_subway'
				? 'Train'
				: 'Bus vehicle'
			: target.kind === 'route'
				? 'Route line'
				: 'Stop';
	if (target.kind === 'stop') return { title: stop?.name ?? 'Stop unavailable', subtitle };
	if (target.kind === 'route')
		return { route, title: route?.long_name || route?.short_name || 'Route unavailable', subtitle };
	const destination = trip ? trip_headsign(trip, route, calls, stops) : undefined;
	const next = calls.find((call) => call.arrival > at);
	const next_stop = next && stops.get(next.stop_id);
	return {
		route,
		title: trip_destination_title(destination, route),
		subtitle,
		detail: !trip ? 'Trip details unavailable' : next_stop ? `Next: ${next_stop.name}` : undefined
	};
}
