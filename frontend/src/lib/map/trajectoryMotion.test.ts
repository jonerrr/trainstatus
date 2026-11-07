import type { GpsEvidence, MotionInterval } from '#lib/client/index.js';

import { expect, test } from 'vitest';

import vectors from '../../../../backend/tests/fixtures/trajectory/motion-vectors.json';
import { evaluateMotion, MotionControllers, validateMotion } from './trajectoryMotion';

const plan = (start = 0, end = 500, t0 = 100, t1 = 200): MotionInterval[] => [
	{ t0, t1, a: start, b: 0, c: 3 * (end - start), d: -2 * (end - start) }
];

test('backend and browser share numerical vectors including joins and holds', () => {
	validateMotion(vectors.motion);
	for (const [time, distance, speed] of vectors.samples) {
		const state = evaluateMotion(vectors.motion, time);
		expect(Math.abs(state.distance - distance)).toBeLessThan(vectors.tolerance);
		expect(Math.abs(state.speed - speed)).toBeLessThan(vectors.tolerance);
	}
	expect(() => validateMotion([{ t0: 0, t1: 1, a: 0, b: 1, c: -3, d: 2 }])).toThrow();
});
test('replacement starts from displayed distance and velocity at receipt time', () => {
	const c = new MotionControllers(),
		old = plan();
	c.sample('trip', 'shape', old, null, 140, false);
	const before = { ...c.sample('trip', 'shape', old, null, 145, false) };
	const revised = plan(0, 500, 100, 230);
	const after = { ...c.sample('trip', 'shape', revised, null, 145, false) };
	expect(after.distance).toBeCloseTo(before.distance, 10);
	expect(after.speed).toBeCloseTo(before.speed, 10);
	let previous = after.distance;
	for (let time = 145; time <= 245; time += 1 / 30) {
		const next = c.sample('trip', 'shape', revised, null, time, false);
		expect(next.distance).toBeGreaterThanOrEqual(previous - 1e-9);
		previous = next.distance;
	}
});
test('repeated delivery cannot renew the absolute GPS correction deadline', () => {
	const c = new MotionControllers();
	c.sample('trip', 'shape', plan(), null, 145, true);
	const evidence: GpsEvidence = {
		vehicle_id: 'bus',
		observed_at: 144,
		distance_m: 300,
		correction_deadline: 155
	};
	for (let time = 145; time <= 156; time += 0.1) {
		// A new decoded plan every poll, same accepted evidence and deadline.
		const motion = plan(300, 450, 144, 180);
		const state = c.sample('trip', 'shape', motion, evidence, time, true);
		if (time >= 155) expect(state.distance).toBeCloseTo(evaluateMotion(motion, time).distance, 7);
	}
});
test('expired GPS evidence resynchronizes immediately and clients share the deadline', () => {
	const evidence: GpsEvidence = {
		vehicle_id: 'bus',
		observed_at: 100,
		distance_m: 300,
		correction_deadline: 110
	};
	const motion = plan(300, 500);
	for (const receipt of [105, 109, 111]) {
		const c = new MotionControllers();
		c.sample('trip', 'shape', plan(), null, receipt - 1, true);
		const at = c.sample('trip', 'shape', motion, evidence, Math.max(receipt, 110), true);
		expect(at.distance).toBeCloseTo(evaluateMotion(motion, Math.max(receipt, 110)).distance, 8);
	}
});
test('reroutes, changed time, long hidden gaps and membership resets discard old display state', () => {
	const c = new MotionControllers(),
		first = plan(),
		second = plan(300, 500);
	c.sample('trip', 'one', first, null, 140, false);
	expect(c.sample('trip', 'two', second, null, 141, false).distance).toBe(
		evaluateMotion(second, 141).distance
	);
	expect(c.sample('trip', 'two', first, null, 100, false).distance).toBe(0);
	expect(c.sample('trip', 'two', second, null, 160, false).distance).toBe(
		evaluateMotion(second, 160).distance
	);
	c.retain(new Set());
	expect(c.sample('trip', 'two', first, null, 161, false).distance).toBe(
		evaluateMotion(first, 161).distance
	);
});

test('brakes before a terminal hold rather than overrunning it', () => {
	const c = new MotionControllers(),
		motion = plan(0, 100, 0, 20);
	c.sample('trip', 'shape', motion, null, 0, false);
	for (let time = 0; time < 60; time += 1 / 30) {
		const state = c.sample('trip', 'shape', motion, null, time, false);
		expect(state.distance).toBeLessThanOrEqual(100 + 1e-6);
		expect(state.speed).toBeGreaterThanOrEqual(0);
	}
});
