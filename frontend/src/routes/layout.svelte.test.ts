import { createRawSnippet, flushSync } from 'svelte';

import { SvelteMap, SvelteURL } from 'svelte/reactivity';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import type { Trip } from '../lib/client';
import { LiveResource } from '../lib/resources/liveResource.svelte';
import { resourceQuery } from '../lib/resources/request';
import { page } from '../lib/test/page.svelte';
import Layout from './+layout.svelte';

const requests = vi.hoisted(() => ({ fail: true }));
const trip: Trip = {
	id: 'requested',
	original_id: 'requested',
	route_id: '1',
	vehicle_id: 'bus',
	created_at: new Date(0),
	updated_at: new Date(0),
	direction: 0,
	shape_ids: [],
	data: { source: 'njt_bus', headsign: 'Destination' }
};
vi.mock('../lib/resources/trips.svelte', async (original) => ({
	...(await original<typeof import('../lib/resources/trips.svelte')>()),
	createTripResource: () =>
		new LiveResource(
			async () => {
				if (requests.fail) throw new Error('Connection failed');
				return new SvelteMap([[trip.id, trip]]);
			},
			new SvelteMap<string, Trip>(),
			{
				query: () => resourceQuery('njt_bus', 'trips'),
				interval: 0
			}
		)
}));
// Only trips are needed to exercise the layout's URL initialization.
vi.mock('../lib/resources/positions.svelte', async (original) => ({
	...(await original<typeof import('../lib/resources/positions.svelte')>()),
	createPositionResource: () => undefined
}));
vi.mock('../lib/resources/stop_times.svelte', async (original) => ({
	...(await original<typeof import('../lib/resources/stop_times.svelte')>()),
	createStopTimeResource: () => undefined
}));
vi.mock('../lib/resources/alerts.svelte', async (original) => ({
	...(await original<typeof import('../lib/resources/alerts.svelte')>()),
	createAlertResource: () => undefined
}));

async function coldLoad() {
	requests.fail = true;
	page.data.selected_sources = ['njt_bus'];
	page.url = new SvelteURL('http://localhost/?t=requested&src=njt_bus');
	return render(Layout, {
		children: createRawSnippet(() => ({ render: () => '<div>Page content</div>' }))
	});
}
test('a direct trip URL opens after shared Retry recovers the data', async () => {
	const view = await coldLoad();
	const retry = view.getByRole('button', { name: /Retry updates/ });
	await expect.element(retry).toBeVisible();
	expect(page.state.modal).toBeNull();
	await expect.element(view.getByText('Trip unavailable.')).not.toBeInTheDocument();
	requests.fail = false;
	await retry.click();
	await expect.poll(() => page.state.modal?.id).toBe('requested');
	await expect.element(view.getByRole('dialog')).toBeVisible();
	await view.unmount();
});
test('reconnect recovery does not open a trip after leaving its URL', async () => {
	const view = await coldLoad();
	await expect.element(view.getByRole('button', { name: /Retry updates/ })).toBeVisible();
	flushSync(() => {
		page.url = new SvelteURL('http://localhost/stops');
	});
	requests.fail = false;
	window.dispatchEvent(new Event('online'));
	await expect.element(view.getByRole('button', { name: /Retry updates/ })).not.toBeInTheDocument();
	expect(page.state.modal).toBeNull();
	await view.unmount();
});
