import '../../app.css';

import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import type { CatalogStop } from '#lib/client/index.js';
import { fixtureFetcher, fixtureTransitData } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';
import { TransitData } from '#lib/transit/store.svelte.js';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Button from './Button.svelte';

test.each(['mta_bus', 'njt_bus'] as const)(
	'%s keeps stop row height when predictions replace skeletons',
	async (source) => {
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		const fixture = chartFixture(source);
		const respond = fixtureFetcher({
			catalogs: { [source]: fixture.catalog },
			services: { [source]: fixture.service }
		});
		let release!: () => void;
		const ready = new Promise<void>((resolve) => {
			release = resolve;
		});
		const data = new TransitData({
			sources: [source],
			batch_ms: 0,
			fetcher: async (input, init) => {
				await ready;
				return respond(input, init);
			}
		});
		data.replaceCatalog(fixture.catalog);
		const view = await render(TransitProvider<{ data: CatalogStop }>, {
			data,
			component: Button,
			child_props: { data: fixture.stops[0] }
		});
		try {
			await expect.poll(() => view.container.querySelectorAll('.animate-pulse').length).toBe(2);
			const height = view.container.getBoundingClientRect().height;
			release();
			await expect.element(view.getByText('1m', { exact: true }).first()).toBeVisible();
			expect(view.container.getBoundingClientRect().height).toBe(height);
		} finally {
			release();
			await view.unmount();
			data.destroy();
			vi.restoreAllMocks();
		}
	}
);

for (const source of ['mta_subway', 'mta_bus', 'njt_bus'] as const) {
	test(`${source} shows a dash placeholder when its route has no trips`, async () => {
		const fixture = chartFixture(source, 0);
		const data = fixtureTransitData({
			catalogs: { [source]: fixture.catalog },
			services: {
				[source]: {
					...fixture.service,
					routes: {
						'4': {
							trips: [],
							stops: [],
							calls: { trip: [], stop: [], sequence: [], arrival: [], departure: [] }
						}
					}
				}
			}
		});
		try {
			const view = await render(TransitProvider<{ data: CatalogStop }>, {
				data,
				component: Button,
				child_props: { data: fixture.stops[0] }
			});
			await expect.element(view.getByText('—', { exact: true }).first()).toBeVisible();
			if (source !== 'mta_subway') {
				await expect
					.element(view.getByText(fixture.route.long_name, { exact: true }))
					.toBeVisible();
			}
			await view.unmount();
		} finally {
			data.destroy();
		}
	});
}

test('shows both arrivals when one trip visits the same stop twice', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const fixture = chartFixture('njt_bus', 0, [['a', 'b', 'a']]);
	const data = fixtureTransitData({
		catalogs: { njt_bus: fixture.catalog },
		services: { njt_bus: fixture.service }
	});
	try {
		const view = await render(TransitProvider<{ data: CatalogStop }>, {
			data,
			component: Button,
			child_props: { data: fixture.stops[0] }
		});
		await expect.element(view.getByText('1m', { exact: true })).toBeVisible();
		await expect.element(view.getByText('3m', { exact: true })).toBeVisible();
		await view.unmount();
	} finally {
		data.destroy();
		vi.restoreAllMocks();
	}
});
