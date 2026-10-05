import type { RequestEvent } from '@sveltejs/kit';

import { afterEach, describe, expect, it, vi } from 'vitest';

import { handleFetch } from './hooks.server';

const event = { url: new URL('https://trainstat.us/') } as RequestEvent;

const private_env = vi.hoisted((): { API_ORIGIN: string | undefined } => ({
	API_ORIGIN: undefined
}));
vi.mock('$app/env/private', () => private_env);

// Echo the request at the transport boundary to check what the hook actually sends.
const echo_fetch: typeof fetch = async (input, init) => {
	const request = new Request(input, init);
	return Response.json({
		url: request.url,
		method: request.method,
		header: request.headers.get('x-test'),
		body: await request.text()
	});
};

afterEach(() => {
	private_env.API_ORIGIN = undefined;
});

describe('internal API routing', () => {
	it('routes same-origin API requests internally while preserving the request', async () => {
		private_env.API_ORIGIN = 'http://trainstatus-backend:3055';
		const response = await handleFetch({
			event,
			request: new Request('https://trainstat.us/api/v1/routes/mta_subway?at=123', {
				method: 'POST',
				headers: { 'x-test': 'preserved' },
				body: 'payload'
			}),
			fetch: echo_fetch
		});
		expect(await response.json()).toEqual({
			url: 'http://trainstatus-backend:3055/api/v1/routes/mta_subway?at=123',
			method: 'POST',
			header: 'preserved',
			body: 'payload'
		});
	});

	it.each([
		'https://other.example/api/v1/routes/mta_subway',
		'https://trainstat.us/martin/tiles',
		'https://trainstat.us/apiary'
	])('leaves unrelated requests unchanged: %s', async (url) => {
		private_env.API_ORIGIN = 'http://trainstatus-backend:3055';
		const response = await handleFetch({ event, request: new Request(url), fetch: echo_fetch });
		expect((await response.json()).url).toBe(url);
	});

	it('uses the existing route when API_ORIGIN is unset', async () => {
		private_env.API_ORIGIN = undefined;
		const url = 'https://trainstat.us/api/v1/stops/mta_subway';
		const response = await handleFetch({ event, request: new Request(url), fetch: echo_fetch });
		expect((await response.json()).url).toBe(url);
	});
});
