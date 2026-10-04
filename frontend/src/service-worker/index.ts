import { version } from '$app/env';
import { assets, immutable } from '$app/manifest';
import { resolve } from '$app/paths';
import { self } from '$app/service-worker';
import type { PathnameWithSearchOrHash } from '$app/types';

// Create a unique cache name for this deployment
const CACHE = `cache-${version}`;

// Manifest paths are relative to the base path, not app route IDs.
// `resolve` prefixes the base so they match `url.pathname`.
function to_pathname(path: string) {
	return resolve(path as PathnameWithSearchOrHash);
}

const ASSETS: string[] = [
	...immutable.map((asset) => to_pathname(asset.path)),
	...assets.map((asset) => to_pathname(asset.path))
];

self.addEventListener('install', (event) => {
	// Create a new cache and add all files to it
	async function addFilesToCache() {
		const cache = await caches.open(CACHE);
		await cache.addAll(ASSETS);
	}

	event.waitUntil(addFilesToCache());
});

self.addEventListener('activate', (event) => {
	// Remove previous cached data from disk
	async function deleteOldCaches() {
		for (const key of await caches.keys()) {
			if (key !== CACHE) await caches.delete(key);
		}
	}

	event.waitUntil(deleteOldCaches());
});

self.addEventListener('fetch', (event) => {
	// ignore POST requests etc
	if (event.request.method !== 'GET') return;

	async function respond() {
		const url = new URL(event.request.url);
		const cache = await caches.open(CACHE);

		// `immutable`/`assets` can always be served from the cache
		if (ASSETS.includes(url.pathname)) {
			const response = await cache.match(url.pathname);

			if (response) {
				return response;
			}
		}

		// for everything else, try the network first, but
		// fall back to the cache if we're offline
		try {
			const response = await fetch(event.request, { signal: AbortSignal.timeout(5000) });

			// if we're offline, fetch can return a value that is not a Response
			// instead of throwing - and we can't pass this non-Response to respondWith
			if (!(response instanceof Response)) {
				throw new Error('invalid response from fetch');
			}

			// if the response is OK and http (prevents caching chrome-extension:// etc)
			if (response.status === 200 && url.protocol.startsWith('http')) {
				cache.put(event.request, response.clone());
			}

			return response;
		} catch (err) {
			const response = await cache.match(event.request);

			if (response) {
				// add header to indicate that this is a fallback
				const headers = new Headers(response.headers);
				headers.append('x-sw-fallback', 'true');

				// return the cached response
				return new Response(response.body, {
					status: response.status,
					statusText: response.statusText,
					headers
				});
			}

			// if there's no cache, then just error out
			// as there is nothing we can do to respond to this request
			throw err;
		}
	}

	event.respondWith(respond());
});
