import type { CatalogStop, Consist, PlatformEdge, ServiceTrip } from '#lib/client/index.js';

export interface ArrivalExit {
	id: string;
	name: string;
	accessible: boolean;
	car: number;
	/** Fraction along the train, from rear (0) to front (1). */
	position: number;
}

export interface ArrivalExits {
	car_count: number;
	exits: ArrivalExit[];
}

/** Only the arrival's assigned edge can supply named boarding guidance. */
export function arrivalExits(
	stop: CatalogStop,
	trip: ServiceTrip,
	edge_ids: readonly string[]
): ArrivalExits | null {
	if (stop.data.source !== 'mta_subway' || trip.data.source !== 'mta_subway') return null;
	const ids = [...new Set(edge_ids)];
	if (ids.length !== 1) return null;
	const edge = stop.data.platform_edges.find((edge) => edge.id === ids[0]);
	if (!edge) return null;
	const consist = trip.data.consist;
	const front = stoppingPosition(edge, trip.direction, consist);
	if (front === null || !consist) return null;
	const length = consist.car_count * consist.car_length_feet;
	const exits: ArrivalExit[] = [];
	for (const point of edge.egress_points) {
		if (
			!Number.isFinite(point.position_ft) ||
			point.position_ft < 0 ||
			point.position_ft > edge.length_ft
		)
			continue;
		const behind_front = (point.position_ft - front) * (trip.direction === 1 ? 1 : -1);
		const car = Math.min(
			consist.car_count,
			Math.max(1, Math.floor(behind_front / consist.car_length_feet) + 1)
		);
		for (const exit of point.street_exits ?? []) {
			const name = exit.intersection.filter((name) => name.trim()).join(' & ');
			if (!name) continue;
			const id = JSON.stringify([point.position_ft, name]);
			const existing = exits.find((entry) => entry.id === id);
			if (existing) {
				existing.accessible ||= exit.accessible;
				continue;
			}
			exits.push({
				id,
				name,
				accessible: exit.accessible,
				car,
				position: Math.max(0, Math.min(1, 1 - behind_front / length))
			});
		}
	}
	return exits.length
		? { car_count: consist.car_count, exits: exits.sort((a, b) => a.position - b.position) }
		: null;
}

function stoppingPosition(
	edge: PlatformEdge,
	direction: number,
	consist: Consist | null | undefined
): number | null {
	if (
		(direction !== 1 && direction !== 3) ||
		!consist ||
		!Number.isInteger(consist.car_count) ||
		consist.car_count <= 0 ||
		!Number.isFinite(consist.car_length_feet) ||
		consist.car_length_feet <= 0
	)
		return null;
	const markers = edge.car_markers.filter(
		(marker) => marker.direction === (direction === 1 ? 'NORTH' : 'SOUTH')
	);
	const length = consist.car_count * consist.car_length_feet;
	const exact = markers.filter(
		(marker) =>
			marker.consist_length_ft != null && Math.abs(marker.consist_length_ft - length) < 0.1
	);
	// An unnumbered S board is the stop for all train lengths. Do not substitute another numbered board.
	const candidates = exact.length
		? exact
		: markers.filter(
				(marker) =>
					marker.consist_length_ft == null && marker.marked_as.trim() === 'S' && !marker.is_opto
			);
	const positions = [...new Set(candidates.map((marker) => marker.position_ft))];
	return positions.length === 1 &&
		Number.isFinite(positions[0]) &&
		positions[0] >= 0 &&
		positions[0] <= edge.length_ft
		? positions[0]
		: null;
}
