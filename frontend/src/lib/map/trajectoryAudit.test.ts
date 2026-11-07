import type { MotionInterval } from '#lib/client/index.js';

import { expect, test } from 'vitest';

import fixture from '../../../../backend/tests/fixtures/trajectory/long-replays.json';
import { renderUnitTableFromIPC } from './trajectoryArrow';
import { TrajectoryAudit } from './trajectoryAudit';
import { trajectoryFixture } from './trajectoryFixture';

function table(motion: MotionInterval[]) {
	const result = renderUnitTableFromIPC(trajectoryFixture());
	const trip = result.units[0].trip;
	trip.shape = {
		key: 'straight',
		positions: [
			[-74, 40.7],
			[-73.8, 40.7]
		],
		distances_m: [0, 20000]
	};
	trip.motion = motion;
	return result;
}
test('audit flags too-fast targets and a bus pinned to a future stop over ten minutes', () => {
	const fast = table([{ t0: 0, t1: 30, a: 0, b: 0, c: 27000, d: -18000 }]);
	const audit = new TrajectoryAudit('mta_subway');
	for (let frame = 0; frame < fixture.duration_s * fixture.frame_hz; frame++)
		audit.sample(fast, 10 + frame / fixture.frame_hz);
	expect(audit.report().flags.some((f) => f.kind === 'target_too_fast')).toBe(true);
	// A target can be bad; displayed motion must still obey physical limits,
	// including its initial frame and the deceleration immediately afterwards.
	expect(audit.report().maxDisplaySpeedMps).toBeLessThanOrEqual(25 + 1e-6);
	expect(
		audit.report().flags.filter((f) => ['display_too_fast', 'refresh_jump'].includes(f.kind))
	).toEqual([]);
	const pinned = table([{ t0: 660, t1: 1000, a: 6000, b: 0, c: 18000, d: -12000 }]);
	pinned.units[0].source = 'njt_bus';
	pinned.trips = new Map([['njt_bus:trip-1', pinned.units[0].trip]]);
	const bus = new TrajectoryAudit('njt_bus');
	for (let frame = 0; frame < fixture.duration_s * fixture.frame_hz; frame++)
		bus.sample(pinned, frame / fixture.frame_hz);
	expect(bus.report().flags.some((f) => f.kind === 'unexpected_hold')).toBe(true);
	expect(bus.report().longestUnexpectedHoldSeconds).toBeGreaterThan(590);
});
test('ten-minute compatible refreshes preserve rendered progression and do not flag declared dwell', () => {
	const audit = new TrajectoryAudit('mta_subway');
	const payload = table([{ t0: 0, t1: 1000, a: 100, b: 0, c: 30000, d: -20000 }]);
	for (let frame = 0; frame < fixture.duration_s * fixture.frame_hz; frame++) {
		if (frame % (fixture.refresh_s * fixture.frame_hz) === 0)
			payload.units[0].trip.motion = payload.units[0].trip.motion.map((p) => ({ ...p }));
		audit.sample(payload, 30 + frame / fixture.frame_hz, () => true);
	}
	expect(audit.report().vehicleFrames).toBe(18000);
	expect(audit.report().flags).toEqual([]);
	expect(audit.report().maxAccelerationMps2).toBeLessThanOrEqual(1.00001);
	const held = table([{ t0: 0, t1: 1000, a: 100, b: 0, c: 0, d: 0 }]);
	const heldAudit = new TrajectoryAudit('mta_subway');
	for (let t = 0; t < 600; t += 1 / 30) heldAudit.sample(held, t, () => false);
	expect(heldAudit.report().flags).toEqual([]);
});
