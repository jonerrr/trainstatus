import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import type { CatalogStop } from '#lib/client/index.js';
import { fixtureTransitData } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Button from './Button.svelte';

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
