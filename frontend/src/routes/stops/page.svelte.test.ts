import { chartFixture } from '#lib/charts/fixtures.js';
import { stop_pins } from '#lib/pins.svelte.js';
import { page } from '#lib/test/page.svelte.js';
import { fixtureTransitData } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';

import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Stops from './+page.svelte';

import '../../app.css';

import type { ComponentProps } from 'svelte';

afterEach(() => vi.restoreAllMocks());

test.each(['mta_subway', 'mta_bus', 'njt_bus'] as const)(
	'%s searches route IDs before conflicting stop IDs and retains stop/name search',
	async (source) => {
		vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true);
		localStorage.removeItem('stopsTab');
		stop_pins.current = { mta_subway: [], mta_bus: [], njt_bus: [] };
		page.data.selected_sources = [source];
		const { catalog, route } = chartFixture(source);
		catalog.routes.push({ ...route, id: '9', short_name: '9' });
		catalog.stops = catalog.stops.slice(0, 3).map((stop, i) => ({
			...stop,
			id: ['4', '100', '200'][i],
			name: ['Unrelated stop', 'First route stop', 'Second route stop'][i],
			routes: [{ ...stop.routes[0], route_id: i === 0 ? '9' : '4', stop_sequence: i }]
		}));
		const data = fixtureTransitData({
			catalogs: { [source]: catalog },
			services: { [source]: { source, routes: {} } }
		});
		const view = await render(TransitProvider<ComponentProps<typeof Stops>>, {
			data,
			component: Stops
		});
		view.container.style.height = '600px';
		try {
			const search = view.getByRole('searchbox');
			await expect.element(view.getByText('Unrelated stop', { exact: true })).toBeVisible();
			await search.fill('4');
			await expect
				.element(view.getByText('Unrelated stop', { exact: true }))
				.not.toBeInTheDocument();
			await expect.element(view.getByText('First route stop', { exact: true })).toBeVisible();
			await expect.element(view.getByText('Second route stop', { exact: true })).toBeVisible();
			await search.fill('9');
			await expect
				.element(view.getByText('First route stop', { exact: true }))
				.not.toBeInTheDocument();
			await expect
				.element(view.getByText('Second route stop', { exact: true }))
				.not.toBeInTheDocument();
			await expect.element(view.getByText('Unrelated stop', { exact: true })).toBeVisible();
			await search.fill('100');
			await expect
				.element(view.getByText('Second route stop', { exact: true }))
				.not.toBeInTheDocument();
			await expect.element(view.getByText('First route stop', { exact: true })).toBeVisible();
			await expect
				.element(view.getByText('Unrelated stop', { exact: true }))
				.not.toBeInTheDocument();
			await search.fill('Second');
			await expect
				.element(view.getByText('First route stop', { exact: true }))
				.not.toBeInTheDocument();
			await expect.element(view.getByText('Second route stop', { exact: true })).toBeVisible();
			await view.getByRole('button', { name: 'Clear search' }).click();
			await expect.element(view.getByText('Unrelated stop', { exact: true })).toBeVisible();
			await expect.element(search).toHaveValue('');
		} finally {
			await view.unmount();
			data.destroy();
		}
	}
);
