import { expect, it } from 'vitest';

import { requestData, resourceQuery } from './request';

it('captures epoch zero and canonical encoded route membership', () => {
	const query = resourceQuery('mta_bus', 'stop_times', 0, ['B+', 'A', 'A']);
	expect(query.routes).toEqual(['A', 'B+']);
	expect(new URL(query.url, 'http://localhost').searchParams.get('at')).toBe('0');
	expect(new URL(query.url, 'http://localhost').searchParams.get('route_ids')).toBe('A,B+');
	expect(Object.isFrozen(query)).toBe(true);
});
it('returns decoded data directly and rejects failed live updates', async () => {
	const signal = new AbortController().signal;
	await expect(
		requestData(
			'/data',
			(response) => response.json(),
			signal,
			async () => new Response('[]')
		)
	).resolves.toEqual([]);
	await expect(
		requestData(
			'/data',
			(response) => response.json(),
			signal,
			async () => new Response('', { status: 503 })
		)
	).rejects.toThrow('503');
	await expect(
		requestData(
			'/data',
			(response) => response.json(),
			signal,
			async () => new Response('invalid')
		)
	).rejects.toBeInstanceOf(SyntaxError);
});
it('cancels a pending fetch with the caller signal', async () => {
	const controller = new AbortController();
	const fetcher: typeof fetch = async (_url, options) => {
		const signal = options?.signal;
		if (!signal) throw new Error('Missing request signal');
		expect(signal).toBe(controller.signal);
		signal.throwIfAborted();
		return new Promise<Response>((_resolve, reject) => {
			signal.addEventListener('abort', () => reject(signal.reason), { once: true });
		});
	};
	const pending = requestData('/data', (res) => res.json(), controller.signal, fetcher);
	controller.abort();
	await expect(pending).rejects.toMatchObject({ name: 'AbortError' });
});
it('preserves the original network failure', async () => {
	const failure = new TypeError('Failed to fetch');
	await expect(
		requestData(
			'/data',
			(response) => response.json(),
			new AbortController().signal,
			async () => {
				throw failure;
			}
		)
	).rejects.toBe(failure);
});
