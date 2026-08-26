import type { Source } from '$lib/client';
import type { ActiveVehicle } from '$lib/map/trajectoryArrow';

/**
 * Single source of truth for what the pointer is currently over.
 */

export interface VehicleHover {
	kind: 'vehicle';
	routeId: string;
	source: string;
	tripId: string;
	vehicle: ActiveVehicle;
	x: number;
	y: number;
}

export interface StopHover {
	kind: 'stop';
	stopId: string;
	source: string;
}

export interface RouteHover {
	kind: 'route';
	routeId: string;
	source: string;
}

export type HoverTarget = VehicleHover | StopHover | RouteHover;

export class MapHover {
	/** Set by deck.gl picking. Highest priority — vehicles draw on top of everything. */
	vehicle = $state<VehicleHover | null>(null);
	/** Set by the stop circle layers. Beats the route line underneath it. */
	stop = $state<StopHover | null>(null);
	/** Set by the route line layer. Lowest priority. */
	route = $state<RouteHover | null>(null);

	readonly active = $derived<HoverTarget | null>(this.vehicle ?? this.stop ?? this.route);

	/** Route to emphasise. Vehicles and stops carry their own route association. */
	readonly routeId = $derived(
		this.active === null || this.active.kind === 'stop' ? null : this.active.routeId
	);

	readonly routeSource = $derived(
		this.active === null || this.active.kind === 'stop' ? null : this.active.source
	);

	readonly tripId = $derived(this.vehicle?.tripId ?? null);

	readonly cursor = $derived<'default' | 'pointer'>(this.active ? 'pointer' : 'default');

	setVehicle(next: VehicleHover | null) {
		this.vehicle = next;
		if (next) {
			// A vehicle sits above the line it runs on, so MapLibre will still be
			// reporting that line as hovered. Drop it so the two never compete.
			this.route = null;
			this.stop = null;
		}
	}

	setStop(next: StopHover | null) {
		if (this.vehicle) return;
		this.stop = next;
		if (next) this.route = null;
	}

	clearStop(stopId: string) {
		if (this.stop?.stopId === stopId) this.stop = null;
	}

	setRoute(next: RouteHover | null) {
		if (this.vehicle || this.stop) return;
		this.route = next;
	}

	clearRoute() {
		this.route = null;
	}

	/**
	 * Drop any hover belonging to a source that is no longer selected. Without
	 * this a hover latched onto a de-selected source can never be cleared, since
	 * its layer stops receiving mouse events entirely.
	 */
	pruneSources(sources: readonly Source[]) {
		const keep = (source: string) => sources.includes(source as Source);
		if (this.vehicle && !keep(this.vehicle.source)) this.vehicle = null;
		if (this.stop && !keep(this.stop.source)) this.stop = null;
		if (this.route && !keep(this.route.source)) this.route = null;
	}
}
