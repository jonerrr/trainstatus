import type { Route, Stop, StopTime, Trip } from '#lib/client/index.js';
import { trip_destination_title, trip_headsign } from '#lib/util.svelte.js';

import type { MapTarget } from './interactions';

export interface FeaturePresentation {
	route?: Route;
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
		stopTimes = [],
		stops = {},
		at = Date.now()
	}: {
		route?: Route;
		stop?: Stop;
		trip?: Trip;
		stopTimes?: readonly StopTime[];
		stops?: Record<string, Stop>;
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
	const destination = trip ? trip_headsign(trip, route, stopTimes, stops) : undefined;
	const next = stopTimes.find((time) => time.arrival.getTime() > at);
	return {
		route,
		title: trip_destination_title(destination, route),
		subtitle,
		detail: !trip
			? 'Trip details unavailable'
			: next && stops[next.stop_id]
				? `Next: ${stops[next.stop_id].name}`
				: undefined
	};
}
