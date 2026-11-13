import type { CatalogRoute, CatalogStop, Source } from '#lib/client/index.js';
import { busHeadsign, mainRouteStops, tripHeadsign } from '#lib/util.svelte.js';

import type { Call, CatalogData } from './decode.js';
import type { RouteRead } from './store.svelte.js';

export interface BoardingStop {
	stop: CatalogStop;
	distanceM: number;
	sameStop: boolean;
}

export interface BoardingOption {
	key: string;
	route: CatalogRoute;
	direction: number | null;
	destination: string;
	stops: BoardingStop[];
	unknownStops: BoardingStop[];
}

export interface IncomingTrip {
	source: Source;
	routeId: string;
	direction: number;
	arrival: number;
}

export interface DepartureOption extends BoardingOption {
	boarding: BoardingStop;
	call: Call;
	next?: DepartureOption;
}

export const stopKey = (stop: CatalogStop) => JSON.stringify([stop.data.source, stop.id]);
export const serviceKey = (source: Source, route: string, direction: number | null) =>
	JSON.stringify([source, route, direction]);

export function boardable(stop: CatalogStop) {
	return stop.routes.length > 0 && (stop.data.source !== 'mta_bus' || stop.data.is_boardable);
}

function memberships(stop: CatalogStop, catalog: CatalogData) {
	return mainRouteStops(stop.routes).flatMap((rs) => {
		const route = catalog.routesById.get(rs.route_id);
		if (!route) return [];
		switch (rs.data.source) {
			case 'mta_bus':
				return (rs.data.direction_ids.length ? rs.data.direction_ids : [null]).map((direction) => ({
					route,
					direction,
					destination:
						direction === null
							? 'Direction unavailable'
							: busHeadsign(route, direction) || 'Destination unavailable'
				}));
			case 'njt_bus':
				return rs.data.directions.map((d) => ({
					route,
					direction: d.direction_id ?? null,
					destination:
						d.direction_id == null
							? 'Direction unavailable'
							: d.headsign || 'Destination unavailable'
				}));
			case 'mta_subway':
				if (stop.data.source !== 'mta_subway') return [];
				return [
					{ route, direction: 1, destination: stop.data.north_headsign || 'Uptown' },
					{ route, direction: 3, destination: stop.data.south_headsign || 'Downtown' }
				];
		}
	});
}

const compareStop = (a: BoardingStop, b: BoardingStop) =>
	a.distanceM - b.distanceM || stopKey(a.stop).localeCompare(stopKey(b.stop));

export function compareOptions(a: BoardingOption, b: BoardingOption) {
	return (
		compareStop(a.stops[0], b.stops[0]) ||
		a.route.id.localeCompare(b.route.id, undefined, { numeric: true }) ||
		a.key.localeCompare(b.key)
	);
}

/** Static boarding choices. Geometry discovers candidates; route-local memberships select badges. */
export function boardingOptions(
	origin: CatalogStop,
	catalogs: readonly CatalogData[],
	incoming?: IncomingTrip
): BoardingOption[] {
	const bySource = new Map(catalogs.map((c) => [c.source, c]));
	const candidates = origin.nearby_stops.flatMap((nearby) => {
		const stop = bySource.get(nearby.source)?.stopsById.get(nearby.stop_id);
		return stop && boardable(stop) ? [{ stop, distanceM: nearby.distance_m, sameStop: false }] : [];
	});
	if (incoming && bySource.has(origin.data.source) && boardable(origin))
		candidates.push({ stop: origin, distanceM: 0, sameStop: true });
	const here = new Set<string>();
	const ownCatalog = bySource.get(origin.data.source);
	if (!incoming && ownCatalog && boardable(origin)) {
		for (const m of memberships(origin, ownCatalog)) {
			if (m.direction !== null) here.add(serviceKey(origin.data.source, m.route.id, m.direction));
		}
	}
	const grouped = new Map<string, BoardingOption>();
	for (const candidate of candidates.sort(compareStop)) {
		const source = candidate.stop.data.source;
		const catalog = bySource.get(source)!;
		for (const m of memberships(candidate.stop, catalog)) {
			const key = serviceKey(source, m.route.id, m.direction);
			const sameStation =
				candidate.sameStop ||
				(origin.data.source === 'mta_subway' &&
					candidate.stop.data.source === 'mta_subway' &&
					origin.data.station_group_id === candidate.stop.data.station_group_id);
			if (
				here.has(key) ||
				(incoming && source === incoming.source && m.route.id === incoming.routeId) ||
				(incoming?.source === 'mta_subway' &&
					source === 'mta_subway' &&
					sameStation &&
					m.direction !== incoming.direction)
			)
				continue;
			let option = grouped.get(key);
			if (!option) {
				option = { key, ...m, stops: [], unknownStops: [] };
				grouped.set(key, option);
			}
			if (!option.stops.some((s) => stopKey(s.stop) === stopKey(candidate.stop)))
				option.stops.push(candidate);
		}
	}
	for (const [key, option] of grouped) {
		if (option.direction !== null) continue;
		const known = [...grouped.values()].filter(
			(o) =>
				o.direction !== null &&
				o.route.data.source === option.route.data.source &&
				o.route.id === option.route.id
		);
		if (known.length) {
			for (const o of known) o.unknownStops = option.stops;
			grouped.delete(key);
		}
	}
	return [...grouped.values()].sort(compareOptions);
}

export function optionStops(option: BoardingOption): BoardingStop[] {
	return [
		...new Map(
			[...option.stops, ...option.unknownStops].sort(compareStop).map((s) => [stopKey(s.stop), s])
		).values()
	];
}

/** Requested routes only: incidental snapshots elsewhere must not change the visible choices. */
export function departureOptions(
	options: readonly BoardingOption[],
	reads: readonly RouteRead[],
	incoming: IncomingTrip | undefined,
	catalogs: readonly CatalogData[],
	referenceMs: number
): DepartureOption[] {
	const results = new Map<string, DepartureOption[]>();
	for (const option of options) {
		const source = option.route.data.source;
		if (source === incoming?.source && option.route.id === incoming.routeId) continue;
		const read = reads.find((r) => r.source === source && r.routeId === option.route.id);
		const route = read?.state === 'ready' ? read.value : undefined;
		const catalog = catalogs.find((c) => c.source === source);
		const matches: DepartureOption[] = [];
		for (const boarding of optionStops(option)) {
			for (const call of route?.callsByStop.get(boarding.stop.id) ?? []) {
				const trip = route?.trips.get(call.tripId);
				if (!trip || call.departure < Math.max(incoming?.arrival ?? referenceMs, referenceMs))
					continue;
				const unknown = option.unknownStops.some((s) => stopKey(s.stop) === stopKey(boarding.stop));
				if (!unknown && option.direction !== null && trip.direction !== option.direction) continue;
				const destination = tripHeadsign(
					trip,
					option.route,
					route?.callsByTrip.get(trip.id),
					catalog?.stopsById
				);
				matches.push({
					...option,
					key: serviceKey(source, option.route.id, trip.direction),
					direction: trip.direction,
					destination: destination === 'Unknown' ? option.destination : destination,
					boarding,
					call
				});
			}
		}
		for (const choice of matches) {
			const group = results.get(choice.key);
			if (group) group.push(choice);
			else results.set(choice.key, [choice]);
		}
	}
	return [...results.values()].map((group) => {
		const sorted = group.sort(compareDepartures);
		const best = sorted[0];
		const next = sorted.find((d) => d.call.tripId !== best.call.tripId);
		return next ? { ...best, next } : best;
	});
}

export function compareDepartures(a: DepartureOption, b: DepartureOption) {
	const time = a.call.departure - b.call.departure;
	return (
		time ||
		compareStop(a.boarding, b.boarding) ||
		a.key.localeCompare(b.key) ||
		a.call.tripId.localeCompare(b.call.tripId) ||
		a.call.sequence - b.call.sequence
	);
}

/** Keep each boarding stop together, nearest first, then show its earliest arrivals. */
export function compareTransfers(a: DepartureOption, b: DepartureOption) {
	return (
		compareStop(a.boarding, b.boarding) ||
		a.call.arrival - b.call.arrival ||
		compareDepartures(a, b)
	);
}

export function boardingDistance(
	distanceM: number,
	sameStop = false,
	locale = new Intl.NumberFormat().resolvedOptions().locale
) {
	if (sameStop) return 'Here';
	const region = new Intl.Locale(locale).maximize().region;
	// CLDR length/road preferences: US feet, UK yards, otherwise metres.
	const unit = region === 'US' ? 'foot' : region === 'GB' ? 'yard' : 'meter';
	const distance = distanceM / (unit === 'foot' ? 0.3048 : unit === 'yard' ? 0.9144 : 1);
	const format = new Intl.NumberFormat(locale, {
		style: 'unit',
		unit,
		unitDisplay: 'short',
		maximumFractionDigits: 0
	});
	return distance < 10
		? `<${format.format(10)}`
		: `~${format.format(Math.round(distance / 10) * 10)}`;
}
