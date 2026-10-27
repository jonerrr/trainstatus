import type {
	CatalogStop,
	Consist,
	EgressPoint,
	PlatformEdge,
	ServiceTrip
} from '#lib/client/index.js';
import type { DepartureOption } from '#lib/transit/nearby.js';

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

/** Only the arrival's assigned edge can supply boarding guidance. */
function arrivalPoints(stop: CatalogStop, trip: ServiceTrip, edge_ids: readonly string[]) {
	if (stop.data.source !== 'mta_subway' || trip.data.source !== 'mta_subway') return null;
	const ids = [...new Set(edge_ids)];
	if (ids.length !== 1) return null;
	const edge = stop.data.platform_edges.find((edge) => edge.id === ids[0]);
	if (!edge) return null;
	const consist = trip.data.consist;
	const front = stoppingPosition(edge, trip.direction, consist);
	if (front === null || !consist) return null;
	const length = consist.car_count * consist.car_length_feet;
	const points = edge.egress_points.flatMap((point) => {
		if (
			!Number.isFinite(point.position_ft) ||
			point.position_ft < 0 ||
			point.position_ft > edge.length_ft
		)
			return [];
		const behind_front = (point.position_ft - front) * (trip.direction === 1 ? 1 : -1);
		return [
			{
				point,
				car: Math.min(
					consist.car_count,
					Math.max(1, Math.floor(behind_front / consist.car_length_feet) + 1)
				),
				position: Math.max(0, Math.min(1, 1 - behind_front / length))
			}
		];
	});
	return { car_count: consist.car_count, points };
}

export function arrivalExits(
	stop: CatalogStop,
	trip: ServiceTrip,
	edge_ids: readonly string[]
): ArrivalExits | null {
	const arrival = arrivalPoints(stop, trip, edge_ids);
	if (!arrival) return null;
	const exits: ArrivalExit[] = [];
	for (const { point, car, position } of arrival.points) {
		for (const exit of point.street_exits ?? []) {
			const name = exit.intersection.filter((name) => name.trim()).join(' & ');
			if (!name) continue;
			const id = JSON.stringify([point.position_ft, name]);
			const existing = exits.find((entry) => entry.id === id);
			if (existing) {
				existing.accessible ||= exit.accessible;
				continue;
			}
			exits.push({ id, name, accessible: exit.accessible, car, position });
		}
	}
	return exits.length
		? { car_count: arrival.car_count, exits: exits.sort((a, b) => a.position - b.position) }
		: null;
}

/** Match the selected departure's actual platform, never a route's usual platform. */
export function arrivalTransferExits(
	stop: CatalogStop,
	trip: ServiceTrip,
	edge_ids: readonly string[],
	departure: DepartureOption
): ArrivalExits | null {
	if (
		departure.route.data.source !== 'mta_subway' ||
		departure.boarding.stop.data.source !== 'mta_subway' ||
		!departure.call
	)
		return null;
	const targets = [...new Set(departure.call.platform_edges)];
	if (
		targets.length !== 1 ||
		!departure.boarding.stop.data.platform_edges.some((edge) => edge.id === targets[0])
	)
		return null;
	const arrival = arrivalPoints(stop, trip, edge_ids);
	if (!arrival) return null;
	const exits = arrival.points.flatMap(({ point, car, position }) => {
		const paths = (point.transfer_paths ?? []).filter(
			(path) => path.to_platform_edge === targets[0]
		);
		return paths.length
			? [
					{
						id: `transfer:${point.id}`,
						name: egressName(point),
						accessible: paths.some((path) => path.accessible),
						car,
						position
					}
				]
			: [];
	});
	return exits.length
		? { car_count: arrival.car_count, exits: exits.sort((a, b) => a.position - b.position) }
		: null;
}

function egressName(point: EgressPoint) {
	switch (point.egress_type) {
		case 'STAIRCASE':
			return 'Stairs';
		case 'ELEVATOR':
			return 'Elevator';
		case 'ESCALATOR':
			return 'Escalator';
		case 'RAMP':
			return 'Ramp';
		case 'DOOR':
			return 'Door';
		case 'FARE_CONTROL':
			return 'Fare control';
		default:
			return 'Passage';
	}
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
