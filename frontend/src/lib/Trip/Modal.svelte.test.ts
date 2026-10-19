import type { ComponentProps } from 'svelte';

import '../../app.css';

import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import { page } from '#lib/test/page.svelte.js';
import { fixtureTransitData } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page as browserPage, userEvent } from 'vitest/browser';

import Modal from './Modal.svelte';

for (const empty of ['zero calls', 'all arrivals passed'] as const) {
	test(`trip details explain ${empty}`, async () => {
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		const fixture = chartFixture('njt_bus', 0, [['a']]);
		const calls = fixture.service.routes['4'].calls;
		if (empty === 'zero calls') {
			for (const column of Object.values(calls)) if (column) column.length = 0;
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
			await expect
				.element(view.getByRole('button', { name: /^Where to sit for/ }))
				.not.toBeInTheDocument();
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

function exitFixture() {
	const fixture = chartFixture('mta_subway', 0, [['a', 'b', 'a']]);
	const stop = fixture.stops[0];
	const trip = fixture.trips[0];
	if (stop.data.source !== 'mta_subway' || trip.data.source !== 'mta_subway')
		throw new Error('Expected subway');
	trip.data.consist = { car_count: 8, car_length_feet: 60 };
	stop.data.platform_edges = [0, 120].map((position_ft, i) => ({
		id: `edge-${i}`,
		length_ft: 600,
		car_markers: [
			{ marked_as: '8', is_opto: false, consist_length_ft: 480, position_ft, direction: 'NORTH' }
		],
		egress_points: [
			{
				id: 1,
				position_ft: 180,
				egress_type: 'STAIRCASE',
				vertical_direction: 'UP',
				street_exits: [{ id: 1, intersection: ['Stuyvesant Av'], accessible: false }]
			},
			{
				id: 2,
				position_ft: 450,
				egress_type: 'ELEVATOR',
				vertical_direction: 'NONE',
				street_exits: [{ id: 2, intersection: ['Utica Av'], accessible: true }]
			}
		]
	}));
	const calls = fixture.service.routes['4'].calls;
	calls.platform_edges = calls.sequence.map((sequence) =>
		sequence === 0 ? ['edge-0'] : sequence === 2 ? ['edge-1'] : []
	);
	return fixture;
}

for (const missing of [false, true]) {
	test(`arrival exits ${missing ? 'hide unavailable guidance' : 'show named exits per visit and support keyboard selection on mobile'}`, async () => {
		await browserPage.viewport(390, 844);
		page.data.selected_sources = ['mta_subway'];
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		const fixture = exitFixture();
		if (missing) fixture.service.routes['4'].calls.platform_edges = [[], [], []];
		const data = fixtureTransitData({
			catalogs: { mta_subway: fixture.catalog },
			services: { mta_subway: fixture.service }
		});
		try {
			const view = await render(TransitProvider<ComponentProps<typeof Modal>>, {
				data,
				component: Modal,
				child_props: {
					source: 'mta_subway',
					route_id: '4',
					trip_id: 'trip-0',
					show_previous: false,
					time_format: 'countdown'
				}
			});
			const toggles = view.getByRole('button', { name: 'Where to sit for a', exact: true });
			await expect.element(view.getByText('8 cars')).toBeVisible();
			if (missing) {
				await expect.element(toggles).not.toBeInTheDocument();
				await expect
					.element(view.getByRole('region', { name: 'Platform exits at arrival' }))
					.not.toBeInTheDocument();
				await expect
					.element(view.getByText(/uncertain|unavailable|Railway north/))
					.not.toBeInTheDocument();
			} else {
				await expect.poll(() => toggles.all().length).toBe(2);
				await toggles.nth(0).click();
				await expect
					.element(
						view.getByRole('button', {
							name: 'Stuyvesant Av, around car 2 from the front',
							exact: true
						})
					)
					.toBeVisible();
				const marker = view.getByRole('button', {
					name: 'Utica Av, accessible exit, around car 6 from the front',
					exact: true
				});
				await marker.click();
				await expect.element(marker).toHaveAttribute('aria-pressed', 'true');
				await userEvent.keyboard('{Tab}{Enter}');
				await expect
					.element(
						view.getByRole('button', {
							name: 'Stuyvesant Av, around car 2 from the front',
							exact: true
						})
					)
					.toHaveAttribute('aria-pressed', 'true');
				await expect
					.element(view.getByRole('status'))
					.toHaveTextContent('Stuyvesant Av · car 2 from the front');
				await expect.element(marker).toHaveAttribute('aria-pressed', 'false');
				await toggles.nth(1).click();
				await expect
					.element(
						view.getByRole('button', {
							name: 'Stuyvesant Av, around car 4 from the front',
							exact: true
						})
					)
					.toBeVisible();
				await expect.element(toggles.nth(0)).toHaveAttribute('aria-expanded', 'false');
				await expect
					.poll(() => view.getByRole('region', { name: 'Platform exits at arrival' }).all().length)
					.toBe(1);
			}
			await view.unmount();
		} finally {
			data.destroy();
			vi.restoreAllMocks();
		}
	});
}
