import { afterEach, expect, test, vi } from 'vitest';

import { watchTrajectories } from './trajectories';

afterEach(() => vi.useRealTimers());

test('a failed historical Arrow update retries until network recovery', async () => {
	vi.useFakeTimers();
	const { trajectoryFixture } = await import('./trajectoryFixture');
	let calls = 0;
	let latest: import('./trajectories').TrajectorySnapshot | undefined;
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: 0, refreshInterval: 100 },
		(snapshot) => {
			latest = snapshot;
		},
		async () => {
			calls++;
			return new Response(trajectoryFixture(), calls === 1 ? { status: 503 } : {});
		}
	);
	await vi.advanceTimersByTimeAsync(0);
	expect(latest?.tables.size).toBe(0);
	expect(latest?.statuses.get('mta_bus')?.available).toBe(false);
	expect(latest?.statuses.get('mta_bus')?.error).toBeInstanceOf(Error);
	await vi.advanceTimersByTimeAsync(1000);
	expect(calls).toBe(2);
	expect(latest?.tables.get('mta_bus')?.table.numRows).toBe(1);
	expect(latest?.statuses.get('mta_bus')?.error).toBeNull();
	await stop.refresh();
	expect(calls).toBe(3);
	stop();
});

test('historical failures retry at the selected time', async () => {
	vi.useFakeTimers();
	const urls: string[] = [];
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: 123, refreshInterval: 100 },
		() => {},
		async (url) => {
			urls.push(String(url));
			return new Response('unavailable', { status: 503 });
		}
	);
	await vi.advanceTimersByTimeAsync(1000);
	expect(urls.length).toBeGreaterThan(1);
	expect(new Set(urls)).toEqual(new Set(['/api/v1/trajectories/mta_bus?at=123']));
	stop();
});

test('disposing a query aborts its requests and rejects late completions', async () => {
	let signal: AbortSignal | null | undefined;
	let finish!: (response: Response) => void;
	const updates: unknown[] = [];
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: null, refreshInterval: 100 },
		(update) => updates.push(update),
		async (_url, init) => {
			signal = init?.signal;
			return new Promise<Response>((resolve) => {
				finish = resolve;
			});
		}
	);
	stop();
	expect(signal?.aborted).toBe(true);
	const before = updates.length;
	finish(new Response('late error', { status: 503 }));
	await new Promise((resolve) => setTimeout(resolve, 0));
	expect(updates).toHaveLength(before);
});

test('live polling reports a source failure and stops after disposal', async () => {
	vi.useFakeTimers();
	let requests = 0;
	const errors: string[] = [];
	const stop = watchTrajectories(
		{ sources: ['njt_bus'], at: null, refreshInterval: 100 },
		(update) => {
			for (const state of update.statuses.values()) {
				if (state.error) errors.push(state.error.message);
			}
		},
		async () => {
			requests++;
			return new Response('', { status: 503 });
		}
	);
	await vi.advanceTimersByTimeAsync(200);
	expect(requests).toBe(3);
	expect(errors.some((error) => error.includes('503'))).toBe(true);
	stop();
	await vi.advanceTimersByTimeAsync(200);
	expect(requests).toBe(3);
});

test('a successful empty snapshot clears vehicles but a failed refresh preserves them', async () => {
	vi.useFakeTimers();
	const { trajectoryFixture } = await import('./trajectoryFixture');
	let requests = 0;
	const counts: number[] = [];
	const stop = watchTrajectories(
		{ sources: ['mta_subway'], at: null, refreshInterval: 100 },
		(snapshot) => {
			counts.push(snapshot.tables.get('mta_subway')?.table.numRows ?? -1);
		},
		async () => {
			requests++;
			if (requests === 2) return new Response('', { status: 503 });
			// Slice a valid IPC stream to zero rows, retaining its schema.
			const { tableFromIPC, tableToIPC } = await import('apache-arrow');
			const bytes = trajectoryFixture();
			return new Response(
				requests === 1 ? bytes : Uint8Array.from(tableToIPC(tableFromIPC(bytes).slice(0, 0)))
			);
		}
	);
	await vi.advanceTimersByTimeAsync(0);
	await vi.waitFor(() => expect(counts.at(-1)).toBe(1));
	await vi.advanceTimersByTimeAsync(100);
	expect(counts.at(-1)).toBe(1);
	await vi.advanceTimersByTimeAsync(100);
	expect(counts.at(-1)).toBe(0);
	stop();
});
