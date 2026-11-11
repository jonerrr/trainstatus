import type { CatalogStop, ServiceTrip } from '#lib/client/index.js';
import type { Call } from '#lib/transit/decode.js';

/**
 * Trip status summary with snake_case fields matching transit domain data conventions.
 */
export interface TripStatusSummary {
	/** Human readable status string (e.g. "At 14 St-Union Sq", "Approaching Times Sq-42 St", "Not yet departed"). */
	description: string;
	/** Target stop name if resolved. */
	stop_name: string | undefined;
	/** ETA to the displayed stop, or null when its call is unavailable / trip completed. */
	eta_minutes: number | null;
	/** ETA label formatted for display (e.g., "2m", "Due", "Now"), or null. */
	eta_label: string | null;
	/** Number of remaining upcoming stops. */
	stops_remaining: number;
}

/**
 * Trip vehicle summary with snake_case fields matching transit domain data conventions.
 */
export interface TripVehicleSummary {
	/** Subway consist details (e.g., "R142A · 10 cars") or bus ID (e.g., "Bus #5842"). */
	vehicle_label: string | undefined;
	/** Schedule deviation string (e.g., "3 min late", "2 min early"). */
	deviation_text: string | undefined;
	/** Color tone for deviation. */
	deviation_tone: 'warning' | 'secondary' | undefined;
}

export function trip_status_summary(
	trip: ServiceTrip,
	calls: readonly Call[],
	stops_by_id: ReadonlyMap<string, CatalogStop> | undefined,
	now_ms: number
): TripStatusSummary {
	// Invariant: backend RouteService and decode.ts guarantee trip calls are strictly ordered by stop_sequence.
	const upcoming = calls.filter((call) => Math.max(call.arrival, call.departure) >= now_ms);
	const next_call = upcoming[0];
	const stops_remaining = upcoming.length;

	const status = trip.status;

	const raw_status =
		status && 'status' in status.data && status.data.status
			? status.data.status.trim().toLowerCase().replace(/[_-]/g, ' ')
			: '';

	const is_unassigned = status?.data.source === 'mta_subway' && status.data.assigned === false;

	let description: string;
	const is_at_stop = raw_status === 'at stop' || raw_status === 'stopped at';
	const is_approaching = raw_status === 'approaching' || raw_status === 'incoming at';
	const is_en_route =
		raw_status === 'en route' || raw_status === 'in transit to' || raw_status === 'in transit';

	const target_call = is_unassigned
		? calls[0]
		: status?.stop_id
			? (upcoming.find((call) => call.stop_id === status.stop_id) ??
				calls.findLast((call) => call.stop_id === status.stop_id))
			: next_call;
	const target_stop_id = is_unassigned
		? target_call?.stop_id
		: status?.stop_id || target_call?.stop_id;
	const stop_name = target_stop_id ? stops_by_id?.get(target_stop_id)?.name : undefined;

	if (is_unassigned) {
		description = stop_name ? `Departs from ${stop_name}` : 'Not yet departed';
	} else if (is_at_stop) {
		description = stop_name ? `At ${stop_name}` : 'At stop';
	} else if (is_approaching) {
		description = stop_name ? `Approaching ${stop_name}` : 'Approaching stop';
	} else if (is_en_route) {
		description = stop_name ? `En route to ${stop_name}` : 'In transit';
	} else if (raw_status) {
		const label = raw_status.charAt(0).toUpperCase() + raw_status.slice(1);
		description = stop_name ? `${label} ${stop_name}` : label;
	} else if (stops_remaining === 0 && calls.length > 0) {
		description = 'Trip completed';
	} else if (stop_name) {
		description = `Approaching ${stop_name}`;
	} else {
		description = 'In service';
	}

	let eta_minutes: number | null = null;
	let eta_label: string | null = null;

	if (target_call) {
		if (is_at_stop) {
			const dep_eta = (target_call.departure - now_ms) / 60_000;
			if (dep_eta > 0.5) {
				eta_minutes = Math.round(dep_eta);
				eta_label = `${eta_minutes}m`;
			} else {
				eta_minutes = 0;
				eta_label = 'Now';
			}
		} else {
			const target_time =
				target_call.arrival >= now_ms ? target_call.arrival : target_call.departure;
			const eta = (target_time - now_ms) / 60_000;
			if (eta <= 0.5) {
				eta_minutes = 0;
				eta_label = 'Due';
			} else {
				eta_minutes = Math.round(eta);
				eta_label = `${eta_minutes}m`;
			}
		}
	}

	return {
		description,
		stop_name,
		eta_minutes,
		eta_label,
		stops_remaining
	};
}

export function trip_vehicle_summary(trip: ServiceTrip): TripVehicleSummary {
	const data = trip.data;

	switch (data.source) {
		case 'mta_subway': {
			const model = data.consist_cars?.[0]?.type;
			const car_count = data.consist?.car_count ?? data.consist_cars?.length;
			const vehicle_label = [
				model,
				car_count ? `${car_count} ${car_count === 1 ? 'car' : 'cars'}` : undefined
			]
				.filter(Boolean)
				.join(' · ');

			return {
				vehicle_label: vehicle_label || undefined,
				deviation_text: undefined,
				deviation_tone: undefined
			};
		}
		case 'mta_bus': {
			const vehicle_label = trip.vehicle_id ? `Bus #${trip.vehicle_id}` : undefined;
			let deviation_text: string | undefined;
			let deviation_tone: 'warning' | 'secondary' | undefined;

			if (data.deviation && Math.abs(data.deviation) > 120) {
				const mins = Math.round(Math.abs(data.deviation) / 60);
				if (data.deviation > 120) {
					deviation_text = `${mins} min late`;
					deviation_tone = 'warning';
				} else if (data.deviation < -120) {
					deviation_text = `${mins} min early`;
					deviation_tone = 'secondary';
				}
			}

			return {
				vehicle_label,
				deviation_text,
				deviation_tone
			};
		}
		case 'njt_bus': {
			const vehicle_label = trip.vehicle_id ? `Bus #${trip.vehicle_id}` : undefined;
			return {
				vehicle_label,
				deviation_text: undefined,
				deviation_tone: undefined
			};
		}
		default: {
			return {
				vehicle_label: undefined,
				deviation_text: undefined,
				deviation_tone: undefined
			};
		}
	}
}
