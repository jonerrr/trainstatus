import type { TrackedUpdate } from '#lib/transit/store.svelte.js';

import { afterEach, expect, test, vi } from 'vitest';

import { watchTrajectories } from './trajectories.svelte.js';

afterEach(() => {
	vi.restoreAllMocks();
	vi.useRealTimers();
});

test('a stalled trajectory request times out and refresh recovers', async () => {
	vi.useFakeTimers();
	const deadline = new AbortController();
	const timeout = vi.spyOn(AbortSignal, 'timeout').mockReturnValueOnce(deadline.signal);
	const { trajectoryFixture } = await import('./trajectoryFixture');
	let requests = 0;
	let latest: import('./trajectories.svelte.js').TrajectorySnapshot | undefined;
	let tracked: TrackedUpdate | undefined;
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: undefined, refreshInterval: 30_000 },
		(snapshot) => (latest = snapshot),
		async (_url, init) => {
			if (++requests > 1) return new Response(trajectoryFixture());
			const signal = init!.signal!;
			return new Promise<Response>((_resolve, reject) => {
				signal.addEventListener('abort', () => reject(signal.reason));
			});
		},
		(update) => {
			tracked = update;
			return () => {};
		}
	);
	try {
		expect(timeout).toHaveBeenCalledWith(15_000);
		deadline.abort(new DOMException('Request timed out', 'TimeoutError'));
		await vi.advanceTimersByTimeAsync(0);
		expect(tracked?.error?.name).toBe('TimeoutError');
		await stop.refresh();
		expect(requests).toBe(2);
		expect(latest?.tables.get('mta_bus')?.units.length).toBe(1);
		expect(tracked?.error).toBeNull();
	} finally {
		stop.destroy();
	}
});

test('a failed historical Arrow update retries until network recovery', async () => {
	vi.useFakeTimers();
	const { trajectoryFixture } = await import('./trajectoryFixture');
	let calls = 0;
	let latest: import('./trajectories.svelte.js').TrajectorySnapshot | undefined;
	let tracked: TrackedUpdate | undefined;
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: 0, refreshInterval: 100 },
		(snapshot) => {
			latest = snapshot;
		},
		async () => {
			calls++;
			return new Response(trajectoryFixture(), calls === 1 ? { status: 503 } : {});
		},
		(update) => {
			tracked = update;
			return () => {};
		}
	);
	await vi.advanceTimersByTimeAsync(0);
	expect(latest?.tables.size).toBe(0);
	expect(tracked?.error).toBeInstanceOf(Error);
	await vi.advanceTimersByTimeAsync(1000);
	expect(calls).toBe(2);
	expect(latest?.tables.get('mta_bus')?.units.length).toBe(1);
	expect(tracked?.error).toBeNull();
	await stop.refresh();
	expect(calls).toBe(3);
	stop.destroy();
});

test('Retry clears fatal Arrow errors synchronously and subsequent failures restore them', async () => {
	const { trajectoryFixture } = await import('./trajectoryFixture');
	let malformed = true;
	let tracked: TrackedUpdate | undefined;
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: 0 },
		() => {},
		async () => new Response(malformed ? 'invalid Arrow' : trajectoryFixture()),
		(update) => {
			tracked = update;
			return () => {};
		}
	);
	try {
		await stop.refresh();
		expect(tracked?.error).toBeInstanceOf(Error);
		expect(tracked?.fatal).toBe(true);
		tracked!.retry();
		expect(tracked?.error).toBeNull();
		expect(tracked?.fatal).toBe(false);
		await stop.refresh();
		expect(tracked?.error).toBeInstanceOf(Error);
		expect(tracked?.fatal).toBe(true);
		malformed = false;
		tracked!.retry();
		expect(tracked?.error).toBeNull();
		expect(tracked?.fatal).toBe(false);
		await stop.refresh();
		expect(tracked?.error).toBeNull();
		expect(tracked?.fatal).toBe(false);
	} finally {
		stop.destroy();
	}
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
	stop.destroy();
});

test('successful historical trajectories ignore live commit age and do not poll again', async () => {
	vi.useFakeTimers();
	const { trajectoryFixture } = await import('./trajectoryFixture');
	let requests = 0;
	let tracked: import('#lib/transit/store.svelte.js').TrackedUpdate | undefined;
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: 0 },
		() => {},
		async () => {
			requests++;
			return new Response(trajectoryFixture(), { headers: { 'x-source-updated-at': '1' } });
		},
		(update) => {
			tracked = update;
			return () => {};
		}
	);
	try {
		await stop.refresh();
		await vi.advanceTimersByTimeAsync(91_000);
		expect(tracked?.source_updated_at).toBeNull();
		expect(tracked?.error).toBeNull();
		expect(requests).toBe(1);
	} finally {
		stop.destroy();
	}
});

test('disposing a query aborts its requests and rejects late completions', async () => {
	let signal: AbortSignal | null | undefined;
	let finish!: (response: Response) => void;
	const updates: unknown[] = [];
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: undefined, refreshInterval: 100 },
		(update) => updates.push(update),
		async (_url, init) => {
			signal = init?.signal;
			return new Promise<Response>((resolve) => {
				finish = resolve;
			});
		}
	);
	stop.destroy();
	expect(signal?.aborted).toBe(true);
	const before = updates.length;
	finish(new Response('late error', { status: 503 }));
	await new Promise((resolve) => setTimeout(resolve, 0));
	expect(updates).toHaveLength(before);
});

test('live polling reports a source failure and stops after disposal', async () => {
	vi.useFakeTimers();
	let requests = 0;
	let tracked: TrackedUpdate | undefined;
	const stop = watchTrajectories(
		{ sources: ['njt_bus'], at: undefined, refreshInterval: 100 },
		() => {},
		async () => {
			requests++;
			return new Response('', { status: 503 });
		},
		(update) => {
			tracked = update;
			return () => {};
		}
	);
	await vi.advanceTimersByTimeAsync(200);
	expect(requests).toBe(3);
	expect(tracked?.error?.message).toContain('503');
	stop.destroy();
	await vi.advanceTimersByTimeAsync(200);
	expect(requests).toBe(3);
});

test('a successful empty snapshot clears vehicles but a failed refresh preserves them', async () => {
	vi.useFakeTimers();
	const { trajectoryFixture } = await import('./trajectoryFixture');
	let requests = 0;
	const counts: number[] = [];
	const stop = watchTrajectories(
		{ sources: ['mta_subway'], at: undefined, refreshInterval: 100 },
		(snapshot) => {
			counts.push(snapshot.tables.get('mta_subway')?.units.length ?? -1);
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
	stop.destroy();
});

test('source changes retain unchanged tables and requests, while time changes discard old data', async () => {
	const { trajectoryFixture } = await import('./trajectoryFixture');
	const requests: { url: string; signal: AbortSignal; finish: (response: Response) => void }[] = [];
	let latest: import('./trajectories.svelte.js').TrajectorySnapshot | undefined;
	const watcher = watchTrajectories(
		{ sources: ['mta_subway', 'mta_bus'], at: 150 },
		(snapshot) => {
			latest = snapshot;
		},
		async (url, init) =>
			new Promise<Response>((finish) => {
				requests.push({ url: String(url), signal: init!.signal!, finish });
			})
	);
	try {
		requests[0].finish(new Response(trajectoryFixture()));
		await vi.waitFor(() => expect(latest?.tables.get('mta_subway')?.units.length).toBe(1));
		const subway = latest!.tables.get('mta_subway');
		watcher.update({ sources: ['mta_subway'], at: 150 });
		expect(requests[0].signal.aborted).toBe(false);
		expect(requests[1].signal.aborted).toBe(true);
		expect(latest!.tables.get('mta_subway')).toBe(subway);
		expect(requests).toHaveLength(2);
		// A completion from the removed bus query cannot restore its table.
		requests[1].finish(new Response(trajectoryFixture(['late-bus'], 200, { source: 'mta_bus' })));
		await Promise.resolve();
		await Promise.resolve();
		expect(latest!.tables.has('mta_bus')).toBe(false);
		watcher.update({ sources: ['mta_subway', 'njt_bus'], at: 150 });
		expect(latest!.tables.get('mta_subway')).toBe(subway);
		expect(requests).toHaveLength(3);
		watcher.update({ sources: ['mta_subway'], at: 175 });
		expect(requests[0].signal.aborted).toBe(true);
		expect(requests[2].signal.aborted).toBe(true);
		expect(latest!.tables.size).toBe(0);
		expect(requests[3].url).toBe('/api/v1/trajectories/mta_subway?at=175');
		requests[3].finish(new Response(trajectoryFixture(['historical'])));
		await watcher.refresh();
		expect(latest!.tables.get('mta_subway')?.units[0].renderUnitId).toBe('historical');
	} finally {
		watcher.destroy();
	}
});
