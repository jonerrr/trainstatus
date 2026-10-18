import type { CatalogRoute, CatalogStop, Source } from '#lib/client/index.js';
import { bus_headsign, main_route_stops, trip_headsign } from '#lib/util.svelte.js';

import type { Call, CatalogData } from './decode.js';
import type { RouteRead } from './store.svelte.js';

export interface BoardingStop {
	stop: CatalogStop;
	distance_m: number;
	same_stop: boolean;
}

export interface BoardingOption {
	key: string;
	route: CatalogRoute;
	direction: number | null;
	destination: string;
	stops: BoardingStop[];
	unknown_stops: BoardingStop[];
}

export interface IncomingTrip {
	source: Source;
	route_id: string;
	arrival: number;
}

export interface DepartureOption extends BoardingOption {
	boarding: BoardingStop;
	call?: Call;
	loading: boolean;
	error: Error | null;
	next?: DepartureOption;
}

export const stopKey = (stop: CatalogStop) => JSON.stringify([stop.data.source, stop.id]);
export const serviceKey = (source: Source, route: string, direction: number | null) =>
	JSON.stringify([source, route, direction]);

export function boardable(stop: CatalogStop) {
	return stop.routes.length > 0 && (stop.data.source !== 'mta_bus' || stop.data.is_boardable);
}

function memberships(stop: CatalogStop, catalog: CatalogData) {
	return main_route_stops(stop.routes).flatMap((rs) => {
		const route = catalog.routes_by_id.get(rs.route_id);
		if (!route) return [];
		switch (rs.data.source) {
			case 'mta_bus':
				return (rs.data.direction_ids.length ? rs.data.direction_ids : [null]).map((direction) => ({
					route,
					direction,
					destination:
						direction === null
							? 'Direction unavailable'
							: bus_headsign(route, direction) || 'Destination unavailable'
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
	a.distance_m - b.distance_m || stopKey(a.stop).localeCompare(stopKey(b.stop));

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
		const stop = bySource.get(nearby.source)?.stops_by_id.get(nearby.stop_id);
		return stop && boardable(stop)
			? [{ stop, distance_m: nearby.distance_m, same_stop: false }]
			: [];
	});
	if (incoming && bySource.has(origin.data.source) && boardable(origin))
		candidates.push({ stop: origin, distance_m: 0, same_stop: true });
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
			if (
				here.has(key) ||
				(incoming && source === incoming.source && m.route.id === incoming.route_id)
			)
				continue;
			let option = grouped.get(key);
			if (!option) {
				option = { key, ...m, stops: [], unknown_stops: [] };
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
			for (const o of known) o.unknown_stops = option.stops;
			grouped.delete(key);
		}
	}
	return [...grouped.values()].sort(compareOptions);
}

export function optionStops(option: BoardingOption): BoardingStop[] {
	return [
		...new Map(
			[...option.stops, ...option.unknown_stops].sort(compareStop).map((s) => [stopKey(s.stop), s])
		).values()
	];
}

/** Requested routes only: incidental snapshots elsewhere must not change the visible choices. */
export function departureOptions(
	options: readonly BoardingOption[],
	reads: readonly RouteRead[],
	incoming: IncomingTrip,
	catalogs: readonly CatalogData[],
	reference_ms: number
): DepartureOption[] {
	const results = new Map<string, DepartureOption[]>();
	for (const option of options) {
		const source = option.route.data.source;
		if (source === incoming.source && option.route.id === incoming.route_id) continue;
		const read = reads.find((r) => r.source === source && r.route_id === option.route.id);
		const catalog = catalogs.find((c) => c.source === source);
		const matches: DepartureOption[] = [];
		for (const boarding of optionStops(option)) {
			for (const call of read?.route?.calls_by_stop.get(boarding.stop.id) ?? []) {
				const trip = read?.route?.trips.get(call.trip_id);
				if (!trip || call.departure < Math.max(incoming.arrival, reference_ms)) continue;
				const unknown = option.unknown_stops.some(
					(s) => stopKey(s.stop) === stopKey(boarding.stop)
				);
				if (!unknown && option.direction !== null && trip.direction !== option.direction) continue;
				const destination = trip_headsign(
					trip,
					option.route,
					read?.route?.calls_by_trip.get(trip.id),
					catalog?.stops_by_id
				);
				matches.push({
					...option,
					key: serviceKey(source, option.route.id, trip.direction),
					direction: trip.direction,
					destination: destination === 'Unknown' ? option.destination : destination,
					boarding,
					call,
					loading: read?.loading ?? true,
					error: read?.error ?? null
				});
			}
		}
		const choices: DepartureOption[] =
			matches.length &&
			(option.direction === null || matches.some((m) => m.direction === option.direction))
				? matches
				: [
						...matches,
						{
							...option,
							boarding: option.stops[0],
							loading: read?.loading ?? true,
							error: read?.error ?? null
						}
					];
		for (const choice of choices) {
			const group = results.get(choice.key);
			if (group) group.push(choice);
			else results.set(choice.key, [choice]);
		}
	}
	return [...results.values()].map((group) => {
		const sorted = group.sort(compareDepartures);
		const best = sorted[0];
		const next = sorted.find((d) => d.call && d.call.trip_id !== best.call?.trip_id);
		return next ? { ...best, next } : best;
	});
}

export function compareDepartures(a: DepartureOption, b: DepartureOption) {
	const time = (a.call?.departure ?? Infinity) - (b.call?.departure ?? Infinity);
	return (
		(Number.isNaN(time) ? 0 : time) ||
		compareStop(a.boarding, b.boarding) ||
		a.key.localeCompare(b.key) ||
		(a.call?.trip_id ?? '').localeCompare(b.call?.trip_id ?? '') ||
		(a.call?.sequence ?? 0) - (b.call?.sequence ?? 0)
	);
}

export function boardingDistance(
	distance_m: number,
	same_stop = false,
	locale = new Intl.NumberFormat().resolvedOptions().locale
) {
	if (same_stop) return 'Here';
	const region = new Intl.Locale(locale).maximize().region;
	// CLDR length/road preferences: US feet, UK yards, otherwise metres.
	const unit = region === 'US' ? 'foot' : region === 'GB' ? 'yard' : 'meter';
	const distance = distance_m / (unit === 'foot' ? 0.3048 : unit === 'yard' ? 0.9144 : 1);
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
