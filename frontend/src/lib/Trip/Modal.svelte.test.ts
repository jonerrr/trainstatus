import type { ComponentProps } from 'svelte';

import '../../app.css';

import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import { page } from '#lib/test/page.svelte.js';
import { fixtureTransitData } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page as browserPage } from 'vitest/browser';

import Modal from './Modal.svelte';

for (const details of ['model and count', 'count only', 'fleet length', 'unavailable'] as const) {
	test(`subway header shows available vehicle details: ${details}`, async () => {
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		page.data.selected_sources = ['mta_subway'];
		const fixture = chartFixture('mta_subway', 0, [['a']]);
		const trip = fixture.trips[0];
		if (trip.data.source !== 'mta_subway') throw new Error('Expected subway');
		trip.data.consist =
			details === 'model and count' || details === 'count only'
				? { car_count: 10, car_length_feet: 60 }
				: undefined;
		trip.data.consist_cars =
			details === 'count only' || details === 'unavailable'
				? []
				: [{ number: '7244', type: details === 'fleet length' ? undefined : 'R142' }];
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
			await expect
				.element(view.getByRole('button', { name: 'Preview a', exact: true }))
				.toBeVisible();
			const expected = {
				'model and count': 'R142 · 10 cars',
				'count only': '10 cars',
				'fleet length': '1 car',
				unavailable: undefined
			}[details];
			if (expected) await expect.element(view.getByText(expected, { exact: true })).toBeVisible();
			else expect(view.container.textContent).not.toMatch(/R142|\bcars?\b|undefined/);
			await view.unmount();
		} finally {
			data.destroy();
			vi.restoreAllMocks();
		}
	});
}

test('shows three same-direction subway connections without expanding the transfer list', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	page.data.selected_sources = ['mta_subway'];
	const f = chartFixture('mta_subway', 0, [['a']]);
	for (const id of ['A', 'B', 'C']) {
		f.catalog.routes.push({ ...f.route, id, short_name: id });
		f.stops[0].routes.push({ ...f.stops[0].routes[0], route_id: id });
		f.service.routes[id] = {
			trips: [
				{ ...f.trips[0], id: `${id}-north`, direction: 1 },
				{ ...f.trips[0], id: `${id}-south`, direction: 3 }
			],
			stops: ['a'],
			calls: {
				trip: [0, 1],
				stop: [0, 0],
				sequence: [0, 0],
				arrival: [chartNow / 1000 + 300, chartNow / 1000 + 240],
				departure: [chartNow / 1000 + 300, chartNow / 1000 + 240]
			}
		};
	}
	const data = fixtureTransitData({
		catalogs: { mta_subway: f.catalog },
		services: { mta_subway: f.service }
	});
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
	try {
		for (const id of ['A', 'B', 'C'])
			await expect
				.element(view.getByRole('button', { name: new RegExp(`^${id} to .*5m`) }))
				.toBeVisible();
		await expect.element(view.getByText('Show more', { exact: true })).not.toBeInTheDocument();
		expect(view.container.querySelectorAll('.transfer-row')).toHaveLength(3);
		await view.getByRole('button', { name: /^C to/ }).click();
		expect(page.state.modal).toMatchObject({ type: 'trip', id: 'C-north' });
	} finally {
		await view.unmount();
		data.destroy();
		vi.restoreAllMocks();
	}
});

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
				await expect
					.element(view.getByRole('button', { name: 'Open a stop modal', exact: true }))
					.toBeVisible();
			}
			await view.unmount();
		} finally {
			data.destroy();
			vi.restoreAllMocks();
		}
	});
}

test('repeated stop visits select one pinned preview with their own arrival times', async () => {
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
		const toggles = view.getByRole('button', { name: 'Preview a', exact: true });
		await expect.poll(() => toggles.all().length).toBe(2);
		await expect.element(toggles.nth(0)).toHaveAttribute('aria-pressed', 'true');
		await expect.element(view.getByRole('button', { name: /^X to .*2m/ })).toBeVisible();
		await toggles.nth(1).click();
		await expect.element(view.getByRole('button', { name: /^X to .*4m/ })).toBeVisible();
		await expect
			.poll(() => view.getByRole('region', { name: 'Selected stop preview' }).all().length)
			.toBe(1);
		await expect.element(toggles.nth(1)).toHaveAttribute('aria-pressed', 'true');
		await expect.element(toggles.nth(0)).toHaveAttribute('aria-pressed', 'false');
		await view.getByRole('button', { name: 'Reset to next stop' }).click();
		await expect.element(toggles.nth(0)).toHaveAttribute('aria-pressed', 'true');
		await view.getByRole('button', { name: 'Open b stop modal' }).click();
		expect(page.state.modal).toMatchObject({ type: 'stop', source: 'njt_bus', id: 'b' });

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
	fixture.catalog.routes.push({ ...fixture.route, id: 'X', short_name: 'X' });
	stop.routes.push({ ...stop.routes[0], route_id: 'X' });
	fixture.service.routes.X = {
		trips: [],
		stops: [],
		calls: { trip: [], stop: [], sequence: [], arrival: [], departure: [] }
	};
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
	test(`pinned train ${missing ? 'omits unavailable exits' : 'follows the selected visit independently of transfer expansion'}`, async () => {
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
			const toggles = view.getByRole('button', { name: 'Preview a', exact: true });
			await expect.element(toggles.nth(0)).toHaveAttribute('aria-pressed', 'true');
			await expect
				.element(view.getByRole('region', { name: 'Platform exits at arrival' }))
				.toBeVisible();
			expect(view.container.querySelectorAll('[data-train-car]')).toHaveLength(8);
			if (missing) {
				await expect.element(view.getByRole('listitem')).not.toBeInTheDocument();
			} else {
				await expect
					.element(
						view.getByRole('listitem', {
							name: 'Stuyvesant Av, around car 4 from the front',
							exact: true
						})
					)
					.toBeVisible();
				await view.getByText('Transfers', { exact: true }).click();
				await toggles.nth(1).click();
				await expect
					.element(
						view.getByRole('listitem', {
							name: 'Stuyvesant Av, around car 2 from the front',
							exact: true
						})
					)
					.toBeVisible();
				const disclosure = view.container.querySelector('details')!;
				expect(disclosure.open).toBe(false);
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

test('transfer car nodes preview matching access points while rows navigate, and refresh removes invalid guidance', async () => {
	await browserPage.viewport(320, 844);
	page.data.selected_sources = ['mta_subway'];
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = exitFixture();
	const stop = f.stops[0];
	if (stop.data.source !== 'mta_subway') throw new Error('Expected subway');
	stop.data.platform_edges[0].egress_points[0].transfer_paths = [
		{ to_platform_edge: 'destination-edge', accessible: false }
	];
	stop.data.platform_edges.push({
		id: 'destination-edge',
		length_ft: 600,
		car_markers: [],
		egress_points: []
	});
	f.service.routes.X = {
		trips: [{ ...f.trips[0], id: 'connecting-trip' }],
		stops: ['a'],
		calls: {
			trip: [0],
			stop: [0],
			sequence: [0],
			arrival: [chartNow / 1000 + 300],
			departure: [chartNow / 1000 + 300],
			platform_edges: [['destination-edge']]
		}
	};
	const data = fixtureTransitData({
		catalogs: { mta_subway: f.catalog },
		services: { mta_subway: f.service }
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
		const preview = view.getByRole('button', { name: /^Preview X to/ });
		await expect.element(preview).toBeVisible();
		await expect.element(preview).toHaveTextContent('4');
		const node = preview.element().querySelector('.car-box')!.getBoundingClientRect();
		const rail = view.container.querySelector('.rail')!.getBoundingClientRect();
		expect(Math.abs((node.left + node.right) / 2 - (rail.left + rail.right) / 2)).toBeLessThan(1);
		const scroller = view
			.getByRole('region', { name: 'Train and platform exits; scroll for more' })
			.element();
		expect(scroller.scrollLeft).toBe(0);
		await preview.click();
		await expect.element(preview).toHaveAttribute('aria-pressed', 'true');
		await expect
			.element(
				view.getByRole('listitem', {
					name: 'Stairs to X, around car 4 from the front',
					exact: true
				})
			)
			.toBeVisible();
		expect(view.container.querySelectorAll('[data-highlighted="true"]')).toHaveLength(1);
		await expect.poll(() => scroller.scrollLeft).toBeGreaterThan(0);
		const car = view.container.querySelector('[data-train-car="4"]')!.getBoundingClientRect();
		const viewport = scroller.getBoundingClientRect();
		expect(car.left).toBeGreaterThanOrEqual(viewport.left);
		expect(car.right).toBeLessThanOrEqual(viewport.right);
		expect(page.state.modal).toBeNull();
		await view.getByRole('button', { name: /^X to/ }).click();
		expect(page.state.modal).toMatchObject({
			type: 'trip',
			source: 'mta_subway',
			route_id: 'X',
			id: 'connecting-trip'
		});
		f.service.routes.X.calls.platform_edges = [['missing-edge']];
		data.retry();
		await expect.element(preview).not.toBeInTheDocument();
		await expect
			.poll(() => view.container.querySelectorAll('[data-highlighted="true"]').length)
			.toBe(0);
		await view.unmount();
	} finally {
		data.destroy();
		vi.restoreAllMocks();
	}
});
