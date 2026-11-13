import type { RequestEvent } from '@sveltejs/kit';

import { afterEach, describe, expect, it, vi } from 'vitest';

import { handleFetch } from './hooks.server';

const event = { url: new URL('https://trainstat.us/') } as RequestEvent;

const privateEnv = vi.hoisted((): Record<'API_ORIGIN', string | undefined> => ({
	['API_ORIGIN']: undefined
}));
vi.mock('$app/env/private', () => privateEnv);

// Echo the request at the transport boundary to check what the hook actually sends.
const echoFetch: typeof fetch = async (input, init) => {
	const request = new Request(input, init);
	return Response.json({
		url: request.url,
		method: request.method,
		header: request.headers.get('x-test'),
		body: await request.text()
	});
};

afterEach(() => {
	privateEnv.API_ORIGIN = undefined;
});

describe('internal API routing', () => {
	it('routes same-origin API requests internally while preserving the request', async () => {
		privateEnv.API_ORIGIN = 'http://trainstatus-backend:3055';
		const response = await handleFetch({
			event,
			request: new Request('https://trainstat.us/api/v1/catalog/mta_subway?at=123', {
				method: 'POST',
				headers: { 'x-test': 'preserved' },
				body: 'payload'
			}),
			fetch: echoFetch
		});
		expect(await response.json()).toEqual({
			url: 'http://trainstatus-backend:3055/api/v1/catalog/mta_subway?at=123',
			method: 'POST',
			header: 'preserved',
			body: 'payload'
		});
	});

	it.each([
		'https://other.example/api/v1/catalog/mta_subway',
		'https://trainstat.us/martin/tiles',
		'https://trainstat.us/apiary'
	])('leaves unrelated requests unchanged: %s', async (url) => {
		privateEnv.API_ORIGIN = 'http://trainstatus-backend:3055';
		const response = await handleFetch({ event, request: new Request(url), fetch: echoFetch });
		expect((await response.json()).url).toBe(url);
	});

	it('uses the existing route when APIORIGIN is unset', async () => {
		privateEnv.API_ORIGIN = undefined;
		const url = 'https://trainstat.us/api/v1/alerts/mta_subway';
		const response = await handleFetch({ event, request: new Request(url), fetch: echoFetch });
		expect((await response.json()).url).toBe(url);
	});
});
