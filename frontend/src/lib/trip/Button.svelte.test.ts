import type { ComponentProps } from 'svelte';

import '../../app.css';

import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import { fixtureTransitData } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';

import { describe, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Button from './Button.svelte';

describe('Trip/Button', () => {
	test('trip row content leaves activation to the enclosing list button', async () => {
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		const fixture = chartFixture('mta_subway', 0, [['a', 'b', 'c']]);
		const data = fixtureTransitData({
			catalogs: { mta_subway: fixture.catalog },
			services: { mta_subway: fixture.service }
		});

		try {
			const view = await render(TransitProvider<ComponentProps<typeof Button>>, {
				data,
				component: Button,
				childProps: {
					source: 'mta_subway',
					routeId: '4',
					tripId: 'trip-0'
				}
			});

			// No inner buttons: click belongs to the enclosing List button
			expect(view.container.querySelector('button,[role="button"]')).toBeNull();
			await view.unmount();
		} finally {
			data.destroy();
			vi.restoreAllMocks();
		}
	});

	test('renders human readable status, destination, and vehicle consist for subway', async () => {
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		const fixture = chartFixture('mta_subway', 0, [['a', 'b', 'c']]);
		const trip = fixture.trips[0];
		if (trip.data.source !== 'mta_subway') throw new Error('Expected subway');

		trip.data.consist = { car_count: 10, car_length_feet: 60 };
		trip.data.consist_cars = [{ number: '7244', type: 'R142A' }];
		trip.status = {
			updated_at: chartNow / 1000,
			stop_id: 'b',
			data: { source: 'mta_subway', assigned: true, status: 'EN_ROUTE' }
		};

		const data = fixtureTransitData({
			catalogs: { mta_subway: fixture.catalog },
			services: { mta_subway: fixture.service }
		});

		try {
			const view = await render(TransitProvider<ComponentProps<typeof Button>>, {
				data,
				component: Button,
				childProps: {
					source: 'mta_subway',
					routeId: '4',
					tripId: 'trip-0'
				}
			});

			// Consist details
			await expect.element(view.getByText('R142A · 10 cars')).toBeVisible();
			// Human-readable status (not raw "EN_ROUTE b")
			await expect.element(view.getByText('En route to b')).toBeVisible();
			expect(view.container.textContent).not.toMatch(/stops left|Last stop/);
			await view.unmount();
		} finally {
			data.destroy();
			vi.restoreAllMocks();
		}
	});

	test('renders bus vehicle ID, delay, and capacity', async () => {
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		const fixture = chartFixture('mta_bus', 0, [['a', 'b', 'c']]);
		const trip = fixture.trips[0];
		if (trip.data.source !== 'mta_bus') throw new Error('Expected bus');

		trip.vehicle_id = '5842';
		trip.data.deviation = 180; // 3 mins late
		trip.status = {
			updated_at: chartNow / 1000,
			stop_id: 'b',
			data: {
				source: 'mta_bus',
				status: 'approaching',
				capacity: 50,
				passengers: 25
			}
		};

		const data = fixtureTransitData({
			catalogs: { mta_bus: fixture.catalog },
			services: { mta_bus: fixture.service }
		});

		try {
			const view = await render(TransitProvider<ComponentProps<typeof Button>>, {
				data,
				component: Button,
				childProps: {
					source: 'mta_bus',
					routeId: '4',
					tripId: 'trip-0'
				}
			});

			await expect.element(view.getByText('Bus #5842')).toBeVisible();
			await expect.element(view.getByText('Approaching b')).toBeVisible();
			await expect.element(view.getByText('3 min late')).toBeVisible();
			await expect.element(view.getByText('25')).toBeVisible(); // Passenger count
			await view.unmount();
		} finally {
			data.destroy();
			vi.restoreAllMocks();
		}
	});

	test('shows Trip not running when trip is not present in service', async () => {
		vi.spyOn(Date, 'now').mockReturnValue(chartNow);
		const fixture = chartFixture('mta_subway', 0, [['a', 'b']]);
		const data = fixtureTransitData({
			catalogs: { mta_subway: fixture.catalog },
			services: { mta_subway: fixture.service }
		});

		try {
			const view = await render(TransitProvider<ComponentProps<typeof Button>>, {
				data,
				component: Button,
				childProps: {
					source: 'mta_subway',
					routeId: '4',
					tripId: 'trip-nonexistent'
				}
			});

			await expect.element(view.getByText('Trip not running')).toBeVisible();
			await view.unmount();
		} finally {
			data.destroy();
			vi.restoreAllMocks();
		}
	});
});
