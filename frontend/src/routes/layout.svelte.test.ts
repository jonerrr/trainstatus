import { createRawSnippet } from 'svelte';

import { SvelteURL } from 'svelte/reactivity';

import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { chartFixture } from '../lib/charts/fixtures';
import { page } from '../lib/test/page.svelte';
import { fixtureFetcher } from '../lib/test/transit-fixture';
import Layout from './+layout.svelte';

let fail = true;

beforeEach(() => {
	fail = true;
	const fixture = chartFixture('njt_bus', 0);
	const respond = fixtureFetcher({
		catalogs: { njt_bus: fixture.catalog },
		services: { njt_bus: fixture.service }
	});
	vi.stubGlobal('fetch', ((input: RequestInfo | URL, init?: RequestInit) =>
		fail
			? Promise.reject(new TypeError('Connection failed'))
			: respond(input, init)) as typeof fetch);
	vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true);
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

async function coldLoad() {
	page.data.selected_sources = ['njt_bus'];
	page.url = new SvelteURL('http://localhost/?t=trip-0&r=4&src=njt_bus');
	return render(Layout, {
		children: createRawSnippet(() => ({ render: () => '<div>Page content</div>' }))
	});
}

test('a direct trip URL opens at once and recovers after Retry inside the modal', async () => {
	const view = await coldLoad();
	await expect
		.poll(() => page.state.modal)
		.toEqual({
			type: 'trip',
			source: 'njt_bus',
			route_id: '4',
			id: 'trip-0'
		});
	const retry = view.getByRole('button', { name: /Retry updates/ });
	await expect.element(retry).toBeVisible();
	await expect.element(view.getByText(/Arrivals unavailable/)).toBeVisible();
	fail = false;
	await view.getByRole('dialog').getByRole('button', { name: 'Retry', exact: true }).click();
	await expect.element(retry).not.toBeInTheDocument();
	await expect.element(view.getByText(/Arrivals unavailable/)).not.toBeInTheDocument();
	await expect.element(view.getByRole('dialog')).toBeVisible();
	await view.unmount();
});

test('a catalog revalidated by the service worker updates the open app', async () => {
	fail = false;
	const view = await coldLoad();
	await expect.element(view.getByRole('button', { name: 'Open route 4' })).toBeVisible();
	const { catalog } = chartFixture('njt_bus', 0);
	const renamed = { ...catalog, routes: [{ ...catalog.routes[0], short_name: 'Z9' }] };
	navigator.serviceWorker.dispatchEvent(
		new MessageEvent('message', { data: { type: 'catalog', body: renamed } })
	);
	await expect.element(view.getByRole('button', { name: 'Open route Z9' })).toBeVisible();
	await view.unmount();
});

test('reconnecting retries failed reads without a click', async () => {
	const view = await coldLoad();
	await expect.element(view.getByRole('button', { name: /Retry updates/ })).toBeVisible();
	fail = false;
	window.dispatchEvent(new Event('online'));
	await expect.element(view.getByRole('button', { name: /Retry updates/ })).not.toBeInTheDocument();
	await view.unmount();
});
