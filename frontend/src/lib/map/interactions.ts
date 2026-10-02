import type { Source } from '$lib/client';
import type { ActiveVehicle } from '$lib/map/trajectoryArrow';

export type MapTargetKind = 'trip' | 'stop' | 'route';

export interface MapTarget {
	kind: MapTargetKind;
	id: string;
	source: Source;
	label: string;
	subtitle?: string;
}

export interface ScreenPoint {
	x: number;
	y: number;
}

export interface VehiclePicker {
	pick(point: ScreenPoint, radius?: number): ActiveVehicle[];
}

export type MapTargetResolution =
	| { kind: 'none' }
	| { kind: 'open'; target: MapTarget }
	| { kind: 'choose'; targets: MapTarget[] };

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

	resolve(point: ScreenPoint, mapTargets: readonly MapTarget[]): MapTargetResolution {
		const vehicleTargets: MapTarget[] = [];
		for (const vehicle of this.#vehiclePicker?.pick(point) ?? []) {
			vehicleTargets.push({
				kind: 'trip',
				id: vehicle.tripId,
				source: vehicle.source,
				label: vehicle.routeId ? `${vehicle.routeId} vehicle` : 'Vehicle',
				subtitle: vehicle.tripId
			});
		}

		return resolveMapTargets([...vehicleTargets, ...mapTargets]);
	}
}

export function resolveMapTargets(targets: readonly MapTarget[]): MapTargetResolution {
	const uniqueTargets = new Map<string, MapTarget>();

	for (const target of targets) {
		uniqueTargets.set(`${target.kind}:${target.source}:${target.id}`, target);
	}

	const resolvedTargets = Array.from(uniqueTargets.values()).sort(
		(a, b) => targetPriority[a.kind] - targetPriority[b.kind]
	);

	if (resolvedTargets.length === 0) return { kind: 'none' };
	if (resolvedTargets.length === 1) return { kind: 'open', target: resolvedTargets[0] };
	return { kind: 'choose', targets: resolvedTargets };
}
