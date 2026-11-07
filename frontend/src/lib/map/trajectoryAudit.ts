import type { Source } from '#lib/client/index.js';

import {
	buildActiveVehiclesAtTime,
	type ActiveVehicle,
	type RenderUnitTable
} from './trajectoryArrow';
import { evaluateMotion, MotionControllers } from './trajectoryMotion';

type FlagKind =
	| 'target_too_fast'
	| 'display_too_fast'
	| 'unexpected_hold'
	| 'display_too_slow'
	| 'reverse'
	| 'refresh_jump'
	| 'gps_correction'
	| 'shape_reset'
	| 'missing_vehicle';
interface Flag {
	routeId: string;
	kind: FlagKind;
	tripId: string;
	time: number;
	observed: number;
	limit: number;
	samples: number;
}
interface Previous {
	time: number;
	distance: number;
	speed: number;
	shape: string;
	slowSince: number | null;
}

/** Bounded diagnostics shared by offline replays and the opt-in live watcher. */
export class TrajectoryAudit {
	private controller = new MotionControllers();
	private pool: ActiveVehicle[] = [];
	private previous = new Map<string, Previous>();
	private routes = new Map<string, string>();
	private table: RenderUnitTable | null = null;
	private flags = new Map<string, Flag>();
	private frames = 0;
	private maxTargetSpeed = 0;
	private maxDisplaySpeed = 0;
	private maxAcceleration = 0;
	private maxHold = 0;
	constructor(
		private source: Source,
		private holdLimit = 30
	) {}
	private flag(kind: FlagKind, tripId: string, time: number, observed: number, limit: number) {
		const key = `${tripId}:${kind}`,
			existing = this.flags.get(key);
		if (existing) {
			existing.samples++;
			existing.observed = Math.max(existing.observed, observed);
		} else
			this.flags.set(key, {
				kind,
				tripId,
				routeId: this.routes.get(tripId) ?? '',
				time,
				observed,
				limit,
				samples: 1
			});
	}
	sample(
		table: RenderUnitTable,
		time: number,
		expectedMoving?: (tripId: string, time: number) => boolean
	) {
		if (this.table !== table) {
			this.routes = new Map(table.units.map((u) => [u.tripId, u.routeId]));
			this.table = table;
		}
		buildActiveVehiclesAtTime(table, time, this.pool, this.controller);
		const visible = new Set(this.pool.map((v) => v.tripId));
		const cap = this.source === 'mta_subway' ? 25 : 30;
		for (const [key, trip] of table.trips) {
			this.frames++;
			const tripId = key.slice(this.source.length + 1);
			const target = evaluateMotion(trip.motion, time);
			const state = this.controller.sample(
				key,
				trip.shape.key,
				trip.motion,
				trip.evidence,
				time,
				this.source !== 'mta_subway'
			);
			this.maxTargetSpeed = Math.max(this.maxTargetSpeed, target.speed);
			this.maxDisplaySpeed = Math.max(this.maxDisplaySpeed, state.speed);
			if (target.speed > cap + 1e-5) this.flag('target_too_fast', tripId, time, target.speed, cap);
			if (state.speed > cap + 1e-5) this.flag('display_too_fast', tripId, time, state.speed, cap);
			const old = this.previous.get(key);
			const moving =
				expectedMoving?.(tripId, time) ??
				((target.speed >= 1 && target.distance >= state.distance - 1) ||
					(this.source !== 'mta_subway' &&
						!trip.evidence &&
						trip.motion[0].a > 100 &&
						time < trip.motion[0].t0));
			let slowSince: number | null = null;
			if (old && time >= old.time) {
				const dt = time - old.time;
				if (old.shape !== trip.shape.key)
					this.flag('shape_reset', tripId, time, Math.abs(state.distance - old.distance), 0);
				else {
					const distance = state.distance - old.distance;
					const actualSpeed = dt > 0 ? distance / dt : 0;
					if (distance < -1e-5)
						this.flag(trip.evidence ? 'gps_correction' : 'reverse', tripId, time, -distance, 0);
					if ((dt === 0 && Math.abs(distance) > 1e-5) || (dt > 0 && actualSpeed > cap + 0.05))
						this.flag(
							trip.evidence ? 'gps_correction' : 'refresh_jump',
							tripId,
							time,
							Math.abs(distance),
							cap * dt
						);
					if (dt > 0)
						this.maxAcceleration = Math.max(
							this.maxAcceleration,
							Math.abs(state.speed - old.speed) / dt
						);
					const minimum = target.speed >= 2 ? Math.min(2, target.speed / 4) : 0.1;
					if (moving && dt > 0 && Math.abs(actualSpeed) < minimum) {
						slowSince = old.slowSince ?? old.time;
						const held = time - slowSince;
						this.maxHold = Math.max(this.maxHold, held);
						if (held > this.holdLimit)
							this.flag(
								Math.abs(actualSpeed) < 0.1 ? 'unexpected_hold' : 'display_too_slow',
								tripId,
								time,
								held,
								this.holdLimit
							);
					}
				}
			}
			if (moving && !visible.has(tripId)) this.flag('missing_vehicle', tripId, time, 1, 0);
			this.previous.set(key, {
				time,
				distance: state.distance,
				speed: state.speed,
				shape: trip.shape.key,
				slowSince
			});
		}
		for (const key of this.previous.keys()) if (!table.trips.has(key)) this.previous.delete(key);
	}
	report() {
		return {
			source: this.source,
			vehicleFrames: this.frames,
			maxTargetSpeedMps: this.maxTargetSpeed,
			maxDisplaySpeedMps: this.maxDisplaySpeed,
			maxAccelerationMps2: this.maxAcceleration,
			longestUnexpectedHoldSeconds: this.maxHold,
			flags: [...this.flags.values()]
		};
	}
}
