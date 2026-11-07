import type { GpsEvidence, MotionInterval } from '#lib/client/index.js';

export interface MotionState {
	distance: number;
	speed: number;
}

export function evaluateMotion(motion: readonly MotionInterval[], t: number): MotionState {
	let lo = 0,
		hi = motion.length;
	while (lo < hi) {
		const mid = (lo + hi) >>> 1;
		if (motion[mid].t1 <= t) lo = mid + 1;
		else hi = mid;
	}
	const p = motion[Math.min(lo, motion.length - 1)];
	const u = Math.max(0, Math.min(1, (t - p.t0) / (p.t1 - p.t0)));
	return {
		distance: p.a + u * (p.b + u * (p.c + u * p.d)),
		speed:
			t <= motion[0].t0 || t >= motion[motion.length - 1].t1
				? 0
				: (p.b + u * (2 * p.c + 3 * p.d * u)) / (p.t1 - p.t0)
	};
}

export function validateMotion(motion: readonly MotionInterval[]): void {
	if (!motion.length) throw new Error('Empty trajectory motion');
	for (let i = 0; i < motion.length; i++) {
		const p = motion[i];
		if (![p.t0, p.t1, p.a, p.b, p.c, p.d].every(Number.isFinite) || p.t1 <= p.t0)
			throw new Error('Invalid trajectory interval');
		const probes = [0, 1];
		if (Math.abs(p.d) > 1e-12) probes.push(Math.max(0, Math.min(1, -p.c / (3 * p.d))));
		if (probes.some((u) => p.b + 2 * p.c * u + 3 * p.d * u * u < -1e-7))
			throw new Error('Reversing trajectory motion');
		if (i) {
			const prev = motion[i - 1];
			if (
				Math.abs(prev.t1 - p.t0) > 1e-6 ||
				Math.abs(prev.a + prev.b + prev.c + prev.d - p.a) > 1e-6 ||
				Math.abs((prev.b + 2 * prev.c + 3 * prev.d) / (prev.t1 - prev.t0) - p.b / (p.t1 - p.t0)) >
					1e-6
			)
				throw new Error('Discontinuous trajectory motion');
		}
	}
	const first = motion[0],
		last = motion[motion.length - 1];
	if (Math.abs(first.b) > 1e-6 || Math.abs(last.b + 2 * last.c + 3 * last.d) > 1e-6)
		throw new Error('Moving trajectory boundary');
}

const limits = {
	bus: { maxSpeed: 30, accel: 1, brake: 1.5 },
	subway: { maxSpeed: 25, accel: 0.8, brake: 1 }
};

interface DisplayState extends MotionState {
	time: number;
	shape: string;
	motion: readonly MotionInterval[];
	evidence: GpsEvidence | null;
}

/** One distance/velocity controller per trip. All cars derive from its head. */
export class MotionControllers {
	private states = new Map<string, DisplayState>();
	clear() {
		this.states.clear();
	}
	retain(keys: ReadonlySet<string>) {
		for (const key of this.states.keys()) if (!keys.has(key)) this.states.delete(key);
	}
	sample(
		key: string,
		shape: string,
		motion: readonly MotionInterval[],
		evidence: GpsEvidence | null,
		time: number,
		bus: boolean
	): MotionState {
		let state = this.states.get(key);
		const target = evaluateMotion(motion, time);
		if (!state || state.shape !== shape || time < state.time || time - state.time > 15) {
			state = {
				...target,
				speed: Math.max(0, Math.min(target.speed, limits[bus ? 'bus' : 'subway'].maxSpeed)),
				time,
				shape,
				motion,
				evidence
			};
			this.states.set(key, state);
			return state;
		}
		// Advance the old display before handing off a replacement at this time.
		this.advance(state, time, bus);
		if (state.motion !== motion) {
			state.motion = motion;
			state.evidence = evidence;
		}
		if (evidence && this.cannotReachDeadline(state, evidence, time, bus)) {
			state.distance = target.distance;
			state.speed = Math.max(0, Math.min(target.speed, limits[bus ? 'bus' : 'subway'].maxSpeed));
		}
		return state;
	}
	private cannotReachDeadline(
		state: DisplayState,
		evidence: GpsEvidence,
		time: number,
		bus: boolean
	): boolean {
		const remaining = evidence.correction_deadline - time;
		if (remaining <= 0) return true;
		const { maxSpeed, accel, brake } = limits[bus ? 'bus' : 'subway'];
		const accelerating = Math.max(0, Math.min(remaining, (maxSpeed - state.speed) / accel));
		const maxDistance =
			state.distance +
			state.speed * accelerating +
			(accel * accelerating * accelerating) / 2 +
			maxSpeed * (remaining - accelerating);
		const braking = Math.min(remaining, state.speed / brake);
		const minDistance = state.distance + state.speed * braking - (brake * braking * braking) / 2;
		const target = evaluateMotion(state.motion, evidence.correction_deadline).distance;
		return target > maxDistance || target < minDistance;
	}
	private advance(state: DisplayState, time: number, bus: boolean) {
		const { maxSpeed, accel, brake } = limits[bus ? 'bus' : 'subway'];
		let remaining = time - state.time;
		while (remaining > 1e-9) {
			const dt = Math.min(remaining, 1 / 30),
				next = state.time + dt;
			const target = evaluateMotion(state.motion, next);
			const evidence = state.evidence;
			const budget = evidence ? evidence.correction_deadline - next : 8;
			const gap = target.distance - state.distance;
			if (evidence && (budget <= 0 || Math.abs(gap) > maxSpeed * Math.max(0, budget))) {
				// Trustworthy evidence wins when bounded correction cannot meet its deadline.
				state.distance = target.distance;
				state.speed = Math.max(0, Math.min(target.speed, limits[bus ? 'bus' : 'subway'].maxSpeed));
			} else {
				let wanted = Math.max(0, Math.min(maxSpeed, target.speed + gap / Math.max(0.25, budget)));
				// Brake before the next plateau/endpoint, rather than stopping at a clamp.
				const hold = state.motion.find(
					(p) =>
						p.t1 >= next &&
						Math.abs(p.b + 2 * p.c + 3 * p.d) < 1e-6 &&
						p.a + p.b + p.c + p.d >= state.distance
				);
				if (hold)
					wanted = Math.min(
						wanted,
						Math.sqrt(
							2 *
								brake *
								Math.max(
									0,
									hold.a +
										hold.b +
										hold.c +
										hold.d -
										state.distance -
										state.speed * dt -
										(accel * dt * dt) / 2
								)
						)
					);
				const speed = Math.max(
					0,
					Math.min(state.speed + accel * dt, Math.max(state.speed - brake * dt, wanted))
				);
				state.distance += ((state.speed + speed) * dt) / 2;
				state.speed = speed;
			}
			state.time = next;
			remaining -= dt;
		}
	}
}
