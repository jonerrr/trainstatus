import type { ComponentProps } from 'svelte';

import '../../app.css';

import { chartNow } from '#lib/charts/fixtures.js';
import { nearbyFixture } from '#lib/test/nearby-fixture.js';
import { page } from '#lib/test/page.svelte.js';
import { fixtureFetcher } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';
import { TransitData } from '#lib/transit/store.svelte.js';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Modal from './Modal.svelte';
import Transfers from './Transfers.svelte';

function fixtureData(
	customize: (f: ReturnType<typeof nearbyFixture>) => void = () => {},
	service_gate?: Promise<void>
) {
	const f = nearbyFixture();
	customize(f);
	page.data.selected_sources = ['mta_bus'];
	const requests: string[] = [];
	const respond = fixtureFetcher({
		catalogs: { mta_bus: f.catalog },
		services: { mta_bus: f.service }
	});
	const data = new TransitData({
		sources: ['mta_bus'],
		batch_ms: 0,
		fetcher: async (input, init) => {
			requests.push(String(input));
			if (String(input).includes('/service/')) await service_gate;
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
	return { ...f, data, requestedRoutes };
}

test('loads visible choices first and preserves proximity order through expansion and collapse', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData();
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		child_props: { stop: f.origin, incoming: f.incoming, time_format: 'countdown' as const }
	});
	try {
		await expect.element(view.getByRole('button', { name: /^A to A North.*5m/ })).toBeVisible();
		expect(f.requestedRoutes()).toEqual(['A', 'B', 'C']);
		expect(view.container.querySelectorAll('button[aria-label*=" to "]')).toHaveLength(3);
		expect(
			view.container.querySelector('button[aria-label^="A to"]')?.getAttribute('aria-label')
		).toContain('Board at far');
		await view.getByRole('button', { name: 'Show more' }).click();
		await expect.element(view.getByRole('button', { name: /^D to D North.*3m/ })).toBeVisible();
		await expect.element(view.getByRole('button', { name: /^A to A North.*20m/ })).toBeVisible();
		expect(f.requestedRoutes()).toEqual(['A', 'B', 'C', 'D']);
		await expect
			.poll(() =>
				[
					...view.container.querySelectorAll(
						'section[aria-label="Transfers after arrival"] > button[aria-label*=" to "]'
					)
				]
					.filter((b) => b.getBoundingClientRect().height > 0)
					.map((b) => b.getAttribute('aria-label')?.split(' ')[0])
			)
			.toEqual(['A', 'B', 'C', 'D']);
		await view.getByRole('button', { name: 'Show less' }).click();
		expect(
			[
				...view.container.querySelectorAll(
					'section[aria-label="Transfers after arrival"] > button[aria-label*=" to "]'
				)
			]
				.filter((b) => b.getBoundingClientRect().height > 0)
				.map((b) => b.getAttribute('aria-label')?.split(' ')[0])
		).toEqual(['A', 'B', 'C']);
		expect(view.container.textContent).not.toContain('Later departures');
		await view.getByRole('button', { name: 'Show more' }).click();
		await view.getByRole('button', { name: /^D to/ }).click();
		expect(page.state.modal).toMatchObject({
			type: 'trip',
			source: 'mta_bus',
			route_id: 'D',
			id: 'D-0'
		});
	} finally {
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});

test('omits empty routes and same-stop distances while keeping expanded choices collapsible', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData((f) => {
		f.origin.routes.push(...f.third.routes);
		f.third.routes = [];
		f.service.routes.C = f.serviceFor('C', [['origin', 8, 0]]);
		f.service.routes.A = f.serviceFor('A', [['far', 5, 0]]);
		f.service.routes.B = f.serviceFor('B', [['far', 0, 0]]);
		f.service.routes.D = f.serviceFor('D', []);
	});
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		child_props: { stop: f.origin, incoming: f.incoming, time_format: 'countdown' as const }
	});
	try {
		const same_stop = view.getByRole('button', { name: /^C to C North.*8m/ });
		await expect.element(same_stop).toBeVisible();
		await expect.element(same_stop).not.toHaveTextContent('Here');
		await expect.element(view.getByRole('button', { name: /^A to A North.*5m/ })).toBeVisible();
		await view.getByRole('button', { name: 'Show more' }).click();
		await expect.poll(() => f.requestedRoutes()).toEqual(['A', 'B', 'C', 'D']);
		await expect
			.poll(() => view.container.querySelectorAll('[aria-label="Loading departures"]').length)
			.toBe(0);
		expect(
			[
				...view.container.querySelectorAll(
					'section[aria-label="Transfers after arrival"] > button[aria-label*=" to "]'
				)
			].map((button) => button.getAttribute('aria-label'))
		).toEqual([expect.stringMatching(/^C to/), expect.stringMatching(/^A to/)]);
		expect(view.container.querySelector('button[aria-label^="B to"]')).toBeNull();
		expect(view.container.querySelector('button[aria-label^="D to"]')).toBeNull();
		expect(view.container.textContent).not.toContain('No later predictions');
		await expect.element(view.getByRole('button', { name: 'Show less' })).toBeVisible();
		await view.getByRole('button', { name: 'Show less' }).click();
		await expect.element(view.getByRole('button', { name: 'Show less' })).not.toBeInTheDocument();
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
		child_props: { stop: f.origin, incoming: f.incoming, time_format: 'countdown' as const }
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

test('closed trip panels advertise static choices without acquiring their service', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData();
	const view = await render(TransitProvider<ComponentProps<typeof Modal>>, {
		data: f.data,
		component: Modal,
		child_props: {
			source: 'mta_bus' as const,
			route_id: 'IN',
			trip_id: 'IN-0',
			show_previous: false,
			time_format: 'countdown' as const
		}
	});
	try {
		await expect
			.element(view.getByRole('button', { name: /Transfers at Board at origin/ }))
			.toBeVisible();
		expect(f.requestedRoutes()).toEqual(['IN']);
		expect(view.container.textContent).not.toContain('Transfers after arrival');
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
		child_props: {
			source: 'mta_bus',
			route_id: 'IN',
			trip_id: 'IN-0',
			show_previous: false,
			time_format: 'countdown'
		}
	});
	try {
		const toggle = view.getByRole('button', { name: /Transfers at Board at origin/ });
		await expect.element(toggle).toBeVisible();
		await toggle.click();
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
		await expect.element(toggle).toHaveAttribute('aria-expanded', 'true');
	} finally {
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});
