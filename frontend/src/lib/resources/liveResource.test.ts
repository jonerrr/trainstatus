import { afterEach, describe, expect, it, vi } from 'vitest';

import { LiveResource } from './liveResource.svelte';
import { resourceQuery, type ResourceQuery } from './request';

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (error: Error) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}
const query = (at: number | null, routes: string[] = []) =>
	resourceQuery('mta_bus', 'stop_times', at, routes);
afterEach(() => vi.useRealTimers());

describe('query publication and waiter contracts', () => {
	it('reports an aborted response body as failure when the query was not cancelled', async () => {
		const failure = new DOMException('Response body aborted', 'AbortError');
		const resource = new LiveResource<string[]>(
			async () => {
				throw failure;
			},
			[],
			{ interval: 0 }
		);
		resource.setQuery(query(null));
		await expect(resource.whenAvailable()).rejects.toBe(failure);
		expect(resource.snapshot.error).toBe(failure);
		expect(resource.snapshot.available).toBe(false);
		resource.dispose();
	});
	it('first-load failure rejects readiness and never promotes the initial empty value', async () => {
		const work = deferred<string[]>();
		const resource = new LiveResource(() => work.promise, [], { interval: 0 });
		resource.setQuery(query(0));
		const ready = resource.whenAvailable().catch((error: Error) => error.message);
		work.reject(new Error('Unavailable'));
		expect(await ready).toBe('Unavailable');
		expect(resource.snapshot.available).toBe(false);
		await expect(resource.whenAvailable()).rejects.toThrow('Unavailable');
		resource.dispose();
	});
	it('requests C after A→B→C and ignores A even when cancellation is ignored', async () => {
		vi.useFakeTimers();
		const calls: Array<{
			query: ResourceQuery;
			signal: AbortSignal;
			work: ReturnType<typeof deferred<string[]>>;
		}> = [];
		const resource = new LiveResource<string[]>(
			(captured, signal) => {
				const work = deferred<string[]>();
				calls.push({ query: captured, signal, work });
				return work.promise;
			},
			[],
			{ debounce: 500, interval: 0 }
		);
		resource.setQuery(query(1));
		await vi.advanceTimersByTimeAsync(500);
		const ready = resource.whenAvailable().catch((error: Error) => error.name);
		resource.setQuery(query(2));
		resource.setQuery(query(3));
		await vi.advanceTimersByTimeAsync(500);
		expect(calls.map((call) => call.query.at)).toEqual([1, 3]);
		expect(calls[0].signal.aborted).toBe(true);
		calls[0].work.resolve(['old']);
		await vi.advanceTimersByTimeAsync(0);
		expect(resource.snapshot.current).toEqual([]);
		expect(await ready).toBe('AbortError');
		calls[1].work.resolve(['current']);
		await vi.advanceTimersByTimeAsync(0);
		expect(resource.snapshot.current).toEqual(['current']);
		expect(resource.snapshot.dataQuery?.at).toBe(3);
		resource.dispose();
	});
	it('deduplicates refreshes and settles them only after publication', async () => {
		const work = deferred<string[]>();
		const fetcher = vi.fn(() => work.promise);
		const resource = new LiveResource(fetcher, [], { interval: 0 });
		resource.setQuery(query(null));
		const first = resource.refresh();
		const second = resource.refresh(true);
		expect(fetcher).toHaveBeenCalledTimes(1);
		work.resolve([]);
		expect(await first).toEqual([]);
		expect(await second).toEqual([]);
		expect(resource.snapshot.available).toBe(true);
		resource.dispose();
	});
	it('preserves route waiters on acquisition and cancels released routes', async () => {
		const works: Array<ReturnType<typeof deferred<string[]>>> = [];
		const resource = new LiveResource<string[]>(
			() => {
				const work = deferred<string[]>();
				works.push(work);
				return work.promise;
			},
			[],
			{ interval: 0, retain: (data) => data }
		);
		resource.setQuery(query(null, ['A']));
		const a = resource.whenAvailable('A');
		resource.setQuery(query(null, ['A', 'B']));
		const b = resource.whenAvailable('B').catch((error: Error) => error.name);
		works[0].resolve(['A']);
		await Promise.resolve();
		expect(resource.coversRoute('B')).toBe(false);
		resource.setQuery(query(null, ['A']));
		expect(await b).toBe('AbortError');
		works[2].resolve(['A-new']);
		expect(await a).toEqual(['A-new']);
		resource.setQuery(query(null, ['A', 'B']));
		expect(resource.coversRoute('A')).toBe(true);
		expect(resource.coversRoute('B')).toBe(false);
		resource.dispose();
	});
	it('cancels waiters on destruction and never publishes or polls afterwards', async () => {
		vi.useFakeTimers();
		const work = deferred<string[]>();
		const publish = vi.fn();
		const fetcher = vi.fn(() => work.promise);
		const resource = new LiveResource(fetcher, [], { interval: 30, onchange: publish });
		resource.setQuery(query(null));
		const ready = resource.whenAvailable().catch((error: Error) => error.name);
		const refresh = resource.refresh().catch((error: Error) => error.name);
		resource.dispose();
		const count = publish.mock.calls.length;
		work.resolve(['late']);
		await vi.advanceTimersByTimeAsync(100);
		expect(await ready).toBe('AbortError');
		expect(await refresh).toBe('AbortError');
		expect(publish).toHaveBeenCalledTimes(count);
		expect(fetcher).toHaveBeenCalledTimes(1);
		expect(resource.snapshot.available).toBe(false);
	});
	it('rejects failures, retains data, and keeps the warning through a retry', async () => {
		let work = deferred<string[]>();
		const resource = new LiveResource(() => work.promise, [], { interval: 0 });
		resource.setQuery(query(null));
		const initial = resource.whenAvailable();
		work.resolve(['good']);
		await initial;
		work = deferred();
		const failed = resource.refresh().catch((error: Error) => error.message);
		work.reject(new Error('failure'));
		expect(await failed).toBe('failure');
		expect(resource.snapshot.current).toEqual(['good']);
		work = deferred();
		const retry = resource.refresh();
		expect(resource.snapshot.error?.message).toBe('failure');
		work.resolve([]);
		await retry;
		expect(resource.snapshot.error).toBeNull();
		expect(resource.snapshot.current).toEqual([]);
		resource.dispose();
	});
	it('historical failures retry until success and then wait for explicit reload', async () => {
		vi.useFakeTimers();
		let calls = 0;
		const resource = new LiveResource<string[]>(
			async () => {
				calls++;
				if (calls === 1) throw new Error('Failed update');
				return ['data'];
			},
			[],
			{ interval: 100 }
		);
		resource.setQuery(query(0));
		await vi.advanceTimersByTimeAsync(0);
		expect(resource.snapshot.available).toBe(false);
		expect(resource.snapshot.error).not.toBeNull();
		await vi.advanceTimersByTimeAsync(1000);
		expect(calls).toBe(2);
		expect(await resource.whenAvailable()).toEqual(['data']);
		expect(resource.snapshot.error).toBeNull();
		await resource.refresh(true);
		expect(calls).toBe(3);
		resource.dispose();
	});
});
