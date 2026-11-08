import type { Source } from '#lib/client/index.js';
import type { ActiveVehicle } from '#lib/map/trajectory.js';

export type MapTargetKind = 'trip' | 'stop' | 'route';

export type MapTarget = { id: string; source: Source } & (
	{ kind: 'trip'; routeId: string } | { kind: 'route' | 'stop' }
);

/** Identity of a route or trip, without a picked feature kind. */
export type MapFeatureKey = Pick<MapTarget, 'id' | 'source'>;

export interface ScreenPoint {
	x: number;
	y: number;
}

export interface VehiclePicker {
	pickClosest(point: ScreenPoint, radius?: number): ActiveVehicle | null;
	pick(point: ScreenPoint, radius?: number): ActiveVehicle[];
}

const targetPriority: Record<MapTargetKind, number> = {
	trip: 0,
	stop: 1,
	route: 2
};

export class MapInteractionController {
	#vehiclePicker: VehiclePicker | null = null;

	registerVehiclePicker(picker: VehiclePicker | null) {
		this.#vehiclePicker = picker;
	}

	resolve(point: ScreenPoint, mapTargets: readonly MapTarget[], closest = false): MapTarget[] {
		const vehicleTargets: MapTarget[] = [];
		const nearest = closest ? this.#vehiclePicker?.pickClosest(point) : null;
		const vehicles = closest
			? nearest
				? [nearest]
				: []
			: (this.#vehiclePicker?.pick(point) ?? []);
		for (const vehicle of vehicles) {
			vehicleTargets.push({
				kind: 'trip',
				id: vehicle.tripId,
				source: vehicle.source,
				routeId: vehicle.routeId
			});
		}

		return resolveMapTargets([...vehicleTargets, ...mapTargets]);
	}
}

export function resolveMapTargets(targets: readonly MapTarget[]): MapTarget[] {
	const uniqueTargets = new Map<string, MapTarget>();

	for (const target of targets) {
		uniqueTargets.set(`${target.kind}:${target.source}:${target.id}`, target);
	}

	return Array.from(uniqueTargets.values()).sort(
		(a, b) => targetPriority[a.kind] - targetPriority[b.kind]
	);
}
