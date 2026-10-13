import type { HandleFetch, HandleServerError } from '@sveltejs/kit/hooks';

import { API_ORIGIN } from '$app/env/private';

import { handleFrontendError } from '#lib/errors.js';

export const handleError: HandleServerError = handleFrontendError;

export const handleFetch: HandleFetch = ({ event, request, fetch }) => {
	const url = new URL(request.url);

	if (
		API_ORIGIN &&
		url.origin === event.url.origin &&
		(url.pathname === '/api' || url.pathname.startsWith('/api/'))
	) {
		// Keep the public origin intact; only SSR API traffic uses the internal backend.
		request = new Request(new URL(url.pathname + url.search, API_ORIGIN), request);
	}

	return fetch(request);
};
