import type { ComponentProps } from 'svelte';

import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import { page } from '#lib/test/page.svelte.js';
import { fixtureTransitData } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Modal from './Modal.svelte';

for (const empty of ['zero calls', 'all arrivals passed'] as const) {
	test(`trip details explain ${empty}`, async () => {
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		const fixture = chartFixture('njt_bus', 0, [['a']]);
		const calls = fixture.service.routes['4'].calls;
		if (empty === 'zero calls') {
			for (const column of Object.values(calls)) column.length = 0;
		} else {
			calls.arrival = [chartNow / 1000 - 90];
			calls.departure = [chartNow / 1000 - 60];
		}
		const data = fixtureTransitData({
			catalogs: { njt_bus: fixture.catalog },
			services: { njt_bus: fixture.service }
		});
		const child_props: ComponentProps<typeof Modal> = {
			source: 'njt_bus',
			route_id: '4',
			trip_id: 'trip-0',
			show_previous: false,
			time_format: 'countdown'
		};
		try {
			const view = await render(TransitProvider<ComponentProps<typeof Modal>>, {
				data,
				component: Modal,
				child_props
			});
			await expect.element(view.getByText('No predictions to show')).toBeVisible();
			if (empty === 'all arrivals passed') {
				await view.rerender({ child_props: { ...child_props, show_previous: true } });
				await expect.element(view.getByText('No predictions to show')).not.toBeInTheDocument();
				await expect.element(view.getByRole('button', { name: /a/ })).toBeVisible();
			}
			await view.unmount();
		} finally {
			data.destroy();
			vi.restoreAllMocks();
		}
	});
}

test('repeated stop visits use their own arrival times while only one transfer panel stays open', async () => {
	page.data.selected_sources = ['njt_bus'];
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const fixture = chartFixture('njt_bus', 0, [['a', 'b', 'a']]);
	fixture.catalog.routes.push({ ...fixture.route, id: 'X', short_name: 'X' });
	fixture.stops[0].routes.push({ ...fixture.stops[0].routes[0], route_id: 'X' });
	fixture.service.routes.X = {
		trips: [
			{ ...fixture.trips[0], id: 'early' },
			{ ...fixture.trips[0], id: 'late' }
		],
		stops: ['a'],
		calls: {
			trip: [0, 1],
			stop: [0, 0],
			sequence: [0, 0],
			arrival: [chartNow / 1000 + 120, chartNow / 1000 + 240],
			departure: [chartNow / 1000 + 120, chartNow / 1000 + 240]
		}
	};
	const data = fixtureTransitData({
		catalogs: { njt_bus: fixture.catalog },
		services: { njt_bus: fixture.service }
	});
	try {
		const view = await render(TransitProvider<ComponentProps<typeof Modal>>, {
			data,
			component: Modal,
			child_props: {
				source: 'njt_bus',
				route_id: '4',
				trip_id: 'trip-0',
				show_previous: false,
				time_format: 'countdown'
			}
		});
		const toggles = view.getByRole('button', { name: /^Transfers at a:/ });
		await expect.poll(() => toggles.all().length).toBe(2);
		await toggles.nth(1).click();
		await expect
			.element(view.getByRole('region', { name: 'Transfers after arrival' }))
			.toBeVisible();
		await expect.element(view.getByRole('button', { name: /^X to .*2m/ })).toBeVisible();
		await expect
			.poll(() => view.getByRole('region', { name: 'Transfers after arrival' }).all().length)
			.toBe(1);
		await toggles.nth(0).click();
		await expect.element(view.getByRole('button', { name: /^X to .*4m/ })).toBeVisible();
		await expect
			.poll(() => view.getByRole('region', { name: 'Transfers after arrival' }).all().length)
			.toBe(1);
		await expect.element(toggles.nth(1)).toHaveAttribute('aria-expanded', 'false');
		await expect.element(toggles.nth(0)).toHaveAttribute('aria-expanded', 'true');
		await view.unmount();
	} finally {
		data.destroy();
		vi.restoreAllMocks();
	}
});
