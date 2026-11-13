import type { ComponentProps } from 'svelte';

import '../../app.css';

import { chartNow } from '#lib/charts/fixtures.js';
import { TransitData } from '#lib/data/store.svelte.js';
import { nearbyFixture } from '#lib/test/nearby-fixture.js';
import { page } from '#lib/test/page.svelte.js';
import { fixtureFetcher } from '#lib/test/transit-fixture.js';
import TransitDataHarness from '#lib/test/TransitDataHarness.svelte';
import TransitProvider from '#lib/test/TransitProvider.svelte';

import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Modal from './Modal.svelte';

afterEach(() => vi.restoreAllMocks());

function fixtureData(kind: 'catalog' | 'service' | null = null) {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	page.data.selectedSources = ['mta_bus'];
	const f = nearbyFixture();
	f.origin.nearby_stops = [];
	f.origin.routes = [...f.far.routes];
	f.service.routes.A = f.serviceFor('A', [['origin', 7, 0]]);
	f.service.routes.B = f.serviceFor('B', [['origin', 10, 0]]);
	const failure = { kind, route: null as string | null };
	const respond = fixtureFetcher({
		catalogs: { mta_bus: f.catalog },
		services: { mta_bus: f.service }
	});
	const data = new TransitData({
		sources: ['mta_bus'],
		batchMs: 0,
		fetcher: async (input, init) => {
			const url = new URL(String(input), location.origin);
			if (
				(failure.kind && url.pathname.includes(`/${failure.kind}/`)) ||
				(failure.route && url.searchParams.get('route_ids')?.split(',').includes(failure.route))
			)
				throw new Error('Unavailable');
			return respond(input, init);
		}
	});
	const props: ComponentProps<typeof Modal> = {
		source: 'mta_bus',
		stopId: f.origin.id,
		showPrevious: false,
		timeFormat: 'countdown'
	};
	return { ...f, data, failure, props };
}

for (const kind of ['catalog', 'service'] as const) {
	test(`a failed stop ${kind} read uses shared error handling and recovers on Retry`, async () => {
		const f = fixtureData(kind);
		const view = await render(TransitProvider<ComponentProps<typeof Modal>>, {
			data: f.data,
			component: Modal,
			childProps: f.props
		});
		try {
			if (kind === 'catalog')
				await expect
					.element(view.getByRole('heading', { name: 'Something went wrong' }))
					.toBeVisible();
			else await expect.poll(() => f.data.status.warning).toBe(true);
			expect(view.container.textContent).not.toContain('unavailable');
			expect(view.container.textContent).not.toContain('No predictions to show');
			f.failure.kind = null;
			if (kind === 'catalog') await view.getByRole('button', { name: 'Try again' }).click();
			else f.data.retry();
			await expect.element(view.getByText('A North', { exact: true })).toBeVisible();
			await expect.element(view.getByText('B North', { exact: true })).toBeVisible();
			await expect.element(view.getByRole('status')).not.toBeInTheDocument();
		} finally {
			await view.unmount();
			f.data.destroy();
		}
	});
}

test('loaded stop predictions remain visible when another route and a later refresh fail', async () => {
	const f = fixtureData();
	const reader = await render(TransitDataHarness, {
		data: f.data,
		source: 'mta_bus',
		readers: [{ id: 'warm', route: 'A' }]
	});
	await expect.element(reader.getByText('A-0', { exact: true })).toBeVisible();
	f.failure.route = 'B';
	const view = await render(TransitProvider<ComponentProps<typeof Modal>>, {
		data: f.data,
		component: Modal,
		childProps: f.props
	});
	try {
		await expect.element(view.getByText('A North', { exact: true })).toBeVisible();
		await expect.poll(() => f.data.status.warning).toBe(true);
		expect(f.data.status.error).toBeNull();
		expect(view.container.textContent).not.toContain('Predictions unavailable');
		f.failure.kind = 'service';
		f.data.retry();
		await expect.poll(() => f.data.status.warning).toBe(true);
		await expect.element(view.getByText('A North', { exact: true })).toBeVisible();
		f.failure.kind = null;
		f.failure.route = null;
		f.data.retry();
		await expect.element(view.getByText('B North', { exact: true })).toBeVisible();
	} finally {
		await view.unmount();
		await reader.unmount();
		f.data.destroy();
	}
});
