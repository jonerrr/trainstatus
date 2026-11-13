import type { ComponentProps } from 'svelte';

import '../app.css';

import { chartFixture, chartNow } from '#lib/charts/fixtures.js';
import { TransitData } from '#lib/data/store.svelte.js';
import List, { type TripItem } from '#lib/List.svelte';
import { page } from '#lib/test/page.svelte.js';
import { fixtureFetcher } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';
import { calculateTripHeight } from '#lib/util.svelte.js';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

for (const source of ['mta_subway', 'mta_bus', 'njt_bus'] as const) {
	test.each([320, 960])(
		`${source} trip rows match virtual offsets at %ipx through loading and scrolling`,
		async (width) => {
			vi.spyOn(Date, 'now').mockReturnValue(chartNow);
			page.data.selectedSources = [source];
			const fixture = chartFixture(
				source,
				0,
				Array.from({ length: 30 }, () => ['a', 'b', 'c'])
			);
			fixture.route.long_name = 'East Harlem–South Ferry via Lexington Avenue';
			for (const stop of fixture.stops) stop.name = 'Grand Central-42 St';
			for (const trip of fixture.trips) {
				trip.vehicle_id = '5842';
				if (trip.data.source === 'mta_subway') {
					trip.data.consist = { car_count: 10, car_length_feet: 60 };
					trip.data.consist_cars = [{ number: '7244', type: 'R142A' }];
				} else if (trip.data.source === 'mta_bus') {
					trip.data.deviation = 180;
					trip.status = {
						updated_at: chartNow / 1000,
						stop_id: 'b',
						data: { source: 'mta_bus', status: 'approaching', passengers: 25, capacity: 50 }
					};
				} else {
					trip.status = {
						updated_at: chartNow / 1000,
						stop_id: 'b',
						data: { source: 'njt_bus', occupancy_status: 'StandingRoomOnly' }
					};
				}
			}
			const items: TripItem[] = fixture.trips.map((trip, i) => ({
				id: trip.id,
				source,
				routeId: '4',
				tripId: i % 3 === 0 ? 'missing-trip' : trip.id
			}));
			const respond = fixtureFetcher({
				catalogs: { [source]: fixture.catalog },
				services: { [source]: fixture.service }
			});
			const { promise: gate, resolve: release } = Promise.withResolvers<void>();
			let serviceRequested = false;
			const data = new TransitData({
				sources: [source],
				batchMs: 0,
				fetcher: async (input, init) => {
					if (String(input).includes('/service/')) {
						serviceRequested = true;
						await gate;
					}
					return respond(input, init);
				}
			});
			const view = await render(TransitProvider<ComponentProps<typeof List<TripItem>>>, {
				data,
				component: List<TripItem>,
				childProps: {
					title: 'Trips',
					type: 'trip',
					sources: { [source]: items },
					heightCalc: calculateTripHeight,
					containerClass: 'h-[240px]',
					overscan: 1
				}
			});
			view.container.style.width = `${width}px`;
			const viewport = view.container.querySelector<HTMLElement>('.overflow-y-auto')!;
			const rows = () => [...view.container.querySelectorAll<HTMLElement>('.list-item')];
			const assertRowGeometry = () => {
				const boxes = rows().map((row) => row.getBoundingClientRect());
				for (const [i, box] of boxes.entries()) {
					expect(box.height).toBeCloseTo(calculateTripHeight(), 5);
					if (i > 0) expect(box.top - boxes[i - 1].top).toBeCloseTo(calculateTripHeight(), 5);
					for (const content of rows()[i].querySelectorAll('span, svg')) {
						const bounds = content.getBoundingClientRect();
						expect(bounds.top).toBeGreaterThanOrEqual(box.top);
						expect(bounds.bottom).toBeLessThanOrEqual(box.bottom);
					}
				}
				expect(viewport.scrollHeight).toBeCloseTo(items.length * calculateTripHeight(), 0);
			};
			try {
				await expect.poll(() => serviceRequested).toBe(true);
				assertRowGeometry();
				release();
				await expect.element(view.getByText('Trip not running').first()).toBeVisible();
				if (source === 'mta_subway') {
					await expect.element(view.getByText('R142A · 10 cars').first()).toBeVisible();
				} else if (source === 'mta_bus') {
					await expect.element(view.getByText('3 min late').first()).toBeVisible();
				} else {
					await expect
						.element(view.getByRole('img', { name: 'Standing room' }).first())
						.toBeVisible();
				}
				assertRowGeometry();

				viewport.scrollTop = calculateTripHeight() * 12.5;
				viewport.dispatchEvent(new Event('scroll'));
				await expect
					.poll(() => rows()[0].getBoundingClientRect().bottom)
					.toBeGreaterThan(viewport.getBoundingClientRect().top - calculateTripHeight() * 2);
				assertRowGeometry();
				const boxes = rows().map((row) => row.getBoundingClientRect());
				expect(boxes[0].top).toBeLessThanOrEqual(viewport.getBoundingClientRect().top);
				expect(boxes.at(-1)!.bottom).toBeGreaterThanOrEqual(
					viewport.getBoundingClientRect().bottom
				);
			} finally {
				release();
				await view.unmount();
				data.destroy();
				vi.restoreAllMocks();
			}
		}
	);
}
