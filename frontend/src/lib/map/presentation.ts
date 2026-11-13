import type { CatalogRoute, CatalogStop, ServiceTrip } from '#lib/client/index.js';
import type { Call } from '#lib/data/decode.js';
import { tripDestinationTitle, tripHeadsign } from '#lib/util.svelte.js';

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
	const destination = trip ? tripHeadsign(trip, route, calls, stops) : undefined;
	const next = calls.find((call) => call.arrival > at);
	const nextStop = next && stops.get(next.stopId);
	return {
		route,
		title: tripDestinationTitle(destination, route),
		subtitle,
		detail: !trip ? 'Trip details unavailable' : nextStop ? `Next: ${nextStop.name}` : undefined
	};
}
