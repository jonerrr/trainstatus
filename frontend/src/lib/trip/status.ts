import type { CatalogStop, ServiceTrip } from '#lib/client/index.js';
import type { Call } from '#lib/data/decode.js';

export interface TripStatusSummary {
	/** Human readable status string (e.g. "At 14 St-Union Sq", "Approaching Times Sq-42 St", "Not yet departed"). */
	description: string;
	/** Target stop name if resolved. */
	stopName: string | undefined;
	/** ETA to the displayed stop, or null when its call is unavailable / trip completed. */
	etaMinutes: number | null;
	/** ETA label formatted for display (e.g., "2m", "Due", "Now"), or null. */
	etaLabel: string | null;
	/** Number of remaining upcoming stops. */
	stopsRemaining: number;
}

export interface TripVehicleSummary {
	/** Subway consist details (e.g., "R142A · 10 cars") or bus ID (e.g., "Bus #5842"). */
	vehicleLabel: string | undefined;
	/** Schedule deviation string (e.g., "3 min late", "2 min early"). */
	deviationText: string | undefined;
	/** Color tone for deviation. */
	deviationTone: 'warning' | 'secondary' | undefined;
}

export function tripStatusSummary(
	trip: ServiceTrip,
	calls: readonly Call[],
	stopsById: ReadonlyMap<string, CatalogStop> | undefined,
	nowMs: number
): TripStatusSummary {
	// Invariant: backend RouteService and decode.ts guarantee trip calls are strictly ordered by stop_sequence.
	const upcoming = calls.filter((call) => Math.max(call.arrival, call.departure) >= nowMs);
	const nextCall = upcoming[0];
	const stopsRemaining = upcoming.length;

	const status = trip.status;

	const rawStatus =
		status && 'status' in status.data && status.data.status
			? status.data.status.trim().toLowerCase().replace(/[_-]/g, ' ')
			: '';

	const isUnassigned = status?.data.source === 'mta_subway' && status.data.assigned === false;

	let description: string;
	const isAtStop = rawStatus === 'at stop' || rawStatus === 'stopped at';
	const isApproaching = rawStatus === 'approaching' || rawStatus === 'incoming at';
	const isEnRoute =
		rawStatus === 'en route' || rawStatus === 'in transit to' || rawStatus === 'in transit';

	const targetCall = isUnassigned
		? calls[0]
		: status?.stop_id
			? (upcoming.find((call) => call.stopId === status.stop_id) ??
				calls.findLast((call) => call.stopId === status.stop_id))
			: nextCall;
	const targetStopId = isUnassigned ? targetCall?.stopId : status?.stop_id || targetCall?.stopId;
	const stopName = targetStopId ? stopsById?.get(targetStopId)?.name : undefined;

	if (isUnassigned) {
		description = stopName ? `Departs from ${stopName}` : 'Not yet departed';
	} else if (isAtStop) {
		description = stopName ? `At ${stopName}` : 'At stop';
	} else if (isApproaching) {
		description = stopName ? `Approaching ${stopName}` : 'Approaching stop';
	} else if (isEnRoute) {
		description = stopName ? `En route to ${stopName}` : 'In transit';
	} else if (rawStatus) {
		const label = rawStatus.charAt(0).toUpperCase() + rawStatus.slice(1);
		description = stopName ? `${label} ${stopName}` : label;
	} else if (stopsRemaining === 0 && calls.length > 0) {
		description = 'Trip completed';
	} else if (stopName) {
		description = `Approaching ${stopName}`;
	} else {
		description = 'In service';
	}

	let etaMinutes: number | null = null;
	let etaLabel: string | null = null;

	if (targetCall) {
		if (isAtStop) {
			const depEta = (targetCall.departure - nowMs) / 60_000;
			if (depEta > 0.5) {
				etaMinutes = Math.round(depEta);
				etaLabel = `${etaMinutes}m`;
			} else {
				etaMinutes = 0;
				etaLabel = 'Now';
			}
		} else {
			const targetTime = targetCall.arrival >= nowMs ? targetCall.arrival : targetCall.departure;
			const eta = (targetTime - nowMs) / 60_000;
			if (eta <= 0.5) {
				etaMinutes = 0;
				etaLabel = 'Due';
			} else {
				etaMinutes = Math.round(eta);
				etaLabel = `${etaMinutes}m`;
			}
		}
	}

	return {
		description,
		stopName,
		etaMinutes,
		etaLabel,
		stopsRemaining
	};
}

export function tripVehicleSummary(trip: ServiceTrip): TripVehicleSummary {
	const data = trip.data;

	switch (data.source) {
		case 'mta_subway': {
			const model = data.consist_cars?.[0]?.type;
			const carCount = data.consist?.car_count ?? data.consist_cars?.length;
			const vehicleLabel = [
				model,
				carCount ? `${carCount} ${carCount === 1 ? 'car' : 'cars'}` : undefined
			]
				.filter(Boolean)
				.join(' · ');

			return {
				vehicleLabel: vehicleLabel || undefined,
				deviationText: undefined,
				deviationTone: undefined
			};
		}
		case 'mta_bus': {
			const vehicleLabel = trip.vehicle_id ? `Bus #${trip.vehicle_id}` : undefined;
			let deviationText: string | undefined;
			let deviationTone: 'warning' | 'secondary' | undefined;

			if (data.deviation && Math.abs(data.deviation) > 120) {
				const mins = Math.round(Math.abs(data.deviation) / 60);
				if (data.deviation > 120) {
					deviationText = `${mins} min late`;
					deviationTone = 'warning';
				} else if (data.deviation < -120) {
					deviationText = `${mins} min early`;
					deviationTone = 'secondary';
				}
			}

			return {
				vehicleLabel,
				deviationText,
				deviationTone
			};
		}
		case 'njt_bus': {
			const vehicleLabel = trip.vehicle_id ? `Bus #${trip.vehicle_id}` : undefined;
			return {
				vehicleLabel,
				deviationText: undefined,
				deviationTone: undefined
			};
		}
		default: {
			return {
				vehicleLabel: undefined,
				deviationText: undefined,
				deviationTone: undefined
			};
		}
	}
}
