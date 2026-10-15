import { version } from '$app/env';
import { assets, immutable, prerendered } from '$app/manifest';
import { resolve } from '$app/paths';
import { self } from '$app/service-worker';
import type { PathnameWithSearchOrHash } from '$app/types';

// Deployment-scoped shell and assets; activation replaces only older shells.
const SHELL_PREFIX = 'shell-';
const SHELL = `${SHELL_PREFIX}${version}`;
// Static catalogs survive deployments; each response is revalidated by ETag.
const CATALOGS = 'catalogs';

// Manifest paths are relative to the base path, not app route IDs.
// `resolve` prefixes the base so they match `url.pathname`.
function to_pathname(path: string) {
	return resolve(path as PathnameWithSearchOrHash);
}

const SHELL_FILES = new Set<string>([
	...immutable.map((file) => to_pathname(file.path)),
	...assets.map((file) => to_pathname(file.path)),
	...prerendered.map((file) => to_pathname(file.path))
]);
const OFFLINE_PAGE = to_pathname('/offline');
const OFFLINE_DATA = to_pathname('/offline/__data.json');
const API = to_pathname('/api/');
const CATALOG_API = to_pathname('/api/v1/catalog/');

self.addEventListener('install', (event) => {
	event.waitUntil(caches.open(SHELL).then((cache) => cache.addAll([...SHELL_FILES])));
});

self.addEventListener('activate', (event) => {
	async function deleteOldShells() {
		for (const key of await caches.keys()) {
			// `cache-*` held every GET response before catalogs had their own cache.
			if ((key.startsWith(SHELL_PREFIX) && key !== SHELL) || key.startsWith('cache-'))
				await caches.delete(key);
		}
	}
	event.waitUntil(deleteOldShells());
});

/** Saved catalog at once, refreshed in the background; network only when none is saved. */
async function catalog(event: FetchEvent): Promise<Response> {
	const cache = await caches.open(CATALOGS);
	const saved = await cache.match(event.request);
	if (saved) {
		event.waitUntil(revalidate(cache, event.request, saved));
		return saved;
	}
	const response = await fetch(event.request);
	if (response.ok) event.waitUntil(cache.put(event.request, response.clone()));
	return response;
}

async function revalidate(cache: Cache, request: Request, saved: Response) {
	const etag = saved.headers.get('etag');
	try {
		const response = await fetch(request.url, {
			cache: 'no-store',
			signal: AbortSignal.timeout(15_000),
			headers: etag ? { 'if-none-match': etag } : {}
		});
		if (!response.ok) return; // 304 keeps the saved body.
		await cache.put(request, response.clone());
		const body = await response.json();
		for (const client of await self.clients.matchAll({ type: 'window' })) {
			client.postMessage({ type: 'catalog', body });
		}
	} catch {
		// Offline or failed revalidation keeps the saved catalog.
	}
}

/** Navigations and route data use the network, falling back to the offline shell. */
async function network_or(event: FetchEvent, fallback: string): Promise<Response> {
	try {
		return await fetch(event.request);
	} catch (error) {
		const shell = await caches.match(fallback);
		if (shell) return shell;
		throw error;
	}
}

self.addEventListener('fetch', (event) => {
	if (event.request.method !== 'GET') return;
	const url = new URL(event.request.url);
	if (url.origin !== self.location.origin) return;

	if (SHELL_FILES.has(url.pathname)) {
		event.respondWith(caches.match(url.pathname).then((saved) => saved ?? fetch(event.request)));
	} else if (url.pathname.startsWith(CATALOG_API)) {
		event.respondWith(catalog(event));
	} else if (url.pathname.startsWith(API)) {
		// Realtime data is never persisted; the app retains it in memory.
		return;
	} else if (event.request.mode === 'navigate') {
		event.respondWith(network_or(event, OFFLINE_PAGE));
	} else if (url.pathname.endsWith('/__data.json')) {
		event.respondWith(network_or(event, OFFLINE_DATA));
	}
});
