import type { ComponentProps } from 'svelte';

import '../../app.css';

import { chartNow } from '#lib/charts/fixtures.js';
import { TransitData } from '#lib/data/store.svelte.js';
import { nearbyFixture } from '#lib/test/nearby-fixture.js';
import { page } from '#lib/test/page.svelte.js';
import { fixtureFetcher } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Modal from './Modal.svelte';
import Transfers from './Transfers.svelte';

function fixtureData(
	customize: (f: ReturnType<typeof nearbyFixture>) => void = () => {},
	serviceGate?: Promise<void>
) {
	const f = nearbyFixture();
	customize(f);
	page.data.selectedSources = ['mta_bus'];
	const requests: string[] = [];
	const serviceState = { fail: false };
	const respond = fixtureFetcher({
		catalogs: { mta_bus: f.catalog },
		services: { mta_bus: f.service }
	});
	const data = new TransitData({
		sources: ['mta_bus'],
		batchMs: 0,
		fetcher: async (input, init) => {
			requests.push(String(input));
			if (String(input).includes('/service/')) {
				await serviceGate;
				if (serviceState.fail) throw new Error('Predictions unavailable');
			}
			return respond(input, init);
		}
	});
	data.replaceCatalog(f.catalog);
	const requestedRoutes = () =>
		[
			...new Set(
				requests
					.filter((url) => url.includes('/service/'))
					.flatMap(
						(url) => new URL(url, location.origin).searchParams.get('route_ids')?.split(',') ?? []
					)
			)
		].sort();
	return { ...f, data, requestedRoutes, serviceState };
}

test('orders by actual boarding stop then arrival through expansion and collapse', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData((f) => {
		f.origin.nearby_stops.find((s) => s.stop_id === f.third.id)!.distance_m = 30;
		f.service.routes.B = f.serviceFor('B', [['far', 3, 0]]);
	});
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		childProps: { stop: f.origin, incoming: f.incoming, timeFormat: 'countdown' as const }
	});
	try {
		await expect.element(view.getByRole('button', { name: /^A to A North.*5m/ })).toBeVisible();
		expect(f.requestedRoutes()).toEqual(['A', 'B', 'C', 'D']);
		expect(view.container.querySelectorAll('button[aria-label*=" to "]')).toHaveLength(3);
		expect(
			view.container.querySelector('button[aria-label^="A to"]')?.getAttribute('aria-label')
		).toContain('Board at far');
		await view.getByText('Show more', { exact: true }).click();
		await expect.element(view.getByRole('button', { name: /^D to D North.*3m/ })).toBeVisible();
		await expect
			.element(view.getByRole('button', { name: /^A to A North.*20m/ }))
			.not.toBeInTheDocument();
		await view.getByText('Later departures', { exact: true }).click();
		await expect.element(view.getByRole('button', { name: /^A to A North.*20m/ })).toBeVisible();
		await view.getByRole('button', { name: 'Collapse later departures', exact: true }).click();
		await expect
			.element(view.getByRole('button', { name: /^A to A North.*20m/ }))
			.not.toBeInTheDocument();
		await expect.element(view.getByRole('button', { name: /^D to D North.*3m/ })).toBeVisible();
		await view.getByText('Later departures', { exact: true }).click();
		await expect.element(view.getByRole('button', { name: /^A to A North.*20m/ })).toBeVisible();
		expect(f.requestedRoutes()).toEqual(['A', 'B', 'C', 'D']);
		await expect
			.poll(() =>
				[
					...view.container.querySelectorAll(
						'section[aria-label="Transfers after arrival"] .transfer-row > button[aria-label*=" to "]'
					)
				]
					.filter(
						(b) =>
							b.getBoundingClientRect().height > 0 && !b.closest('[aria-label="Later departures"]')
					)
					.map((b) => b.getAttribute('aria-label')?.split(' ')[0])
			)
			.toEqual(['C', 'B', 'A', 'D']);
		await view.getByText('Show less', { exact: true }).click();
		expect(
			[
				...view.container.querySelectorAll(
					'section[aria-label="Transfers after arrival"] .transfer-row > button[aria-label*=" to "]'
				)
			]
				.filter(
					(b) =>
						b.getBoundingClientRect().height > 0 && !b.closest('[aria-label="Later departures"]')
				)
				.map((b) => b.getAttribute('aria-label')?.split(' ')[0])
		).toEqual(['C', 'B', 'A']);
		expect(view.container.textContent).not.toContain('Later departures');
		await view.getByText('Show more', { exact: true }).click();
		await view.getByRole('button', { name: /^D to/ }).click();
		expect(page.state.modal).toMatchObject({
			type: 'trip',
			source: 'mta_bus',
			routeId: 'D',
			id: 'D-0'
		});
	} finally {
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});

test('fills three rows from available departures beyond empty choices and omits same-stop distances', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData((f) => {
		f.origin.routes.push(...f.third.routes);
		f.third.routes = [];
		f.service.routes.C = f.serviceFor('C', [['origin', 8, 0]]);
		f.service.routes.A = f.serviceFor('A', [['far', 5, 0]]);
		f.service.routes.B = f.serviceFor('B', [['far', 0, 0]]);
		f.service.routes.D = f.serviceFor('D', [['fourth', 3, 0]]);
	});
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		childProps: { stop: f.origin, incoming: f.incoming, timeFormat: 'countdown' as const }
	});
	try {
		const sameStop = view.getByRole('button', { name: /^C to C North.*8m/ });
		await expect.element(sameStop).toBeVisible();
		await expect.element(sameStop).not.toHaveTextContent('Here');
		await expect.element(view.getByRole('button', { name: /^A to A North.*5m/ })).toBeVisible();
		await expect.element(view.getByRole('button', { name: /^D to D North.*3m/ })).toBeVisible();
		await expect.poll(() => f.requestedRoutes()).toEqual(['A', 'B', 'C', 'D']);
		await expect
			.poll(() => view.container.querySelectorAll('[aria-label="Loading departures"]').length)
			.toBe(0);
		expect(
			[
				...view.container.querySelectorAll(
					'section[aria-label="Transfers after arrival"] .transfer-row > button[aria-label*=" to "]'
				)
			].map((button) => button.getAttribute('aria-label'))
		).toEqual([
			expect.stringMatching(/^C to/),
			expect.stringMatching(/^A to/),
			expect.stringMatching(/^D to/)
		]);
		expect(view.container.querySelector('button[aria-label^="B to"]')).toBeNull();
		expect(view.container.textContent).not.toContain('No later predictions');
		await expect.element(view.getByText('Show more', { exact: true })).not.toBeInTheDocument();
	} finally {
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});

test('does not flash route rows while bus predictions load and resolve empty', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	const f = fixtureData((f) => {
		f.far.routes = [];
		f.third.routes = [];
		f.fourth.routes = [];
		f.service.routes.A = f.serviceFor('A', []);
	}, gate);
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		childProps: { stop: f.origin, incoming: f.incoming, timeFormat: 'countdown' as const }
	});
	try {
		await expect.poll(() => f.requestedRoutes()).toEqual(['A']);
		await expect.element(view.getByRole('status')).toHaveTextContent('Loading predictions…');
		expect(view.container.querySelector('button[aria-label*=" to "]')).toBeNull();
		release();
		await expect
			.element(view.getByRole('status'))
			.toHaveTextContent('No transfer predictions to show');
		expect(view.container.querySelector('button[aria-label*=" to "]')).toBeNull();
	} finally {
		release();
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});

test('hides failed transfers with known options and restores departures on Retry', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData();
	f.serviceState.fail = true;
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		childProps: { stop: f.origin, incoming: f.incoming, timeFormat: 'countdown' }
	});
	try {
		await expect.poll(() => f.data.status.warning).toBe(true);
		await expect
			.element(view.getByRole('region', { name: 'Transfers after arrival' }))
			.not.toBeInTheDocument();
		expect(f.requestedRoutes()).toEqual(['A', 'B', 'C', 'D']);
		f.serviceState.fail = false;
		f.data.retry();
		await expect.element(view.getByRole('button', { name: /^A to/ }).first()).toBeVisible();
		await expect
			.element(view.getByRole('region', { name: 'Transfers after arrival' }))
			.toBeVisible();
	} finally {
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});

test('a collapsed transfer section advertises choices without acquiring route service', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData();
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		childProps: { stop: f.origin, incoming: f.incoming, timeFormat: 'countdown', open: false }
	});
	try {
		await expect.element(view.getByText('Transfers', { exact: true })).toBeVisible();
		expect(f.requestedRoutes()).toEqual([]);
		await view.getByText('Transfers', { exact: true }).click();
		await expect.element(view.getByRole('button', { name: /^A to/ })).toBeVisible();
		expect(f.requestedRoutes()).toEqual(['A', 'B', 'C', 'D']);
	} finally {
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});

test('an open transfer panel follows refreshed incoming arrival predictions', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData();
	const view = await render(TransitProvider<ComponentProps<typeof Modal>>, {
		data: f.data,
		component: Modal,
		childProps: {
			source: 'mta_bus',
			routeId: 'IN',
			tripId: 'IN-0',
			showPrevious: false,
			timeFormat: 'countdown'
		}
	});
	try {
		const toggle = view.getByRole('button', { name: 'Preview Board at origin', exact: true });
		await expect.element(toggle).toBeVisible();
		await expect.element(view.getByRole('button', { name: /^A to A North.*5m/ })).toBeVisible();
		f.service.routes.IN.calls.arrival[0] = chartNow / 1000 + 6 * 60;
		f.service.routes.IN.calls.departure[0] = chartNow / 1000 + 6 * 60;
		f.data.retry();
		await expect
			.element(view.getByRole('button', { name: /^A to A North.*20m, 14 minutes after arrival/ }))
			.toBeVisible();
		await expect
			.element(view.getByRole('button', { name: /^A to A North.*5m/ }))
			.not.toBeInTheDocument();
		await expect.element(toggle).toHaveAttribute('aria-pressed', 'true');
	} finally {
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});
