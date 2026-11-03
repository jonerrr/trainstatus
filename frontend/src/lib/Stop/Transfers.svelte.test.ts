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

import Transfers from './Transfers.svelte';

function fixtureData(
	customize: (f: ReturnType<typeof nearbyFixture>) => void = () => {},
	service_gate?: Promise<void>
) {
	const f = nearbyFixture();
	customize(f);
	page.data.selected_sources = ['mta_bus'];
	const requests: string[] = [];
	const service_state = { fail: false };
	const respond = fixtureFetcher({
		catalogs: { mta_bus: f.catalog },
		services: { mta_bus: f.service }
	});
	const data = new TransitData({
		sources: ['mta_bus'],
		batch_ms: 0,
		fetcher: async (input, init) => {
			requests.push(String(input));
			if (String(input).includes('/service/')) {
				await service_gate;
				if (service_state.fail) throw new Error('Predictions unavailable');
			}
			return respond(input, init);
		}
	});
	data.replaceCatalog(f.catalog);
	return { ...f, data, requests, service_state };
}

test('shows compact predictions in an expandable thread and opens the actual boarding stop', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData((f) => {
		f.far.name = f.origin.name;
	});
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		child_props: { stop: f.origin }
	});
	try {
		await expect.element(view.getByRole('button', { name: /^A to A North.*5m/ })).toBeVisible();
		const same_name = view.container.querySelector('button[aria-label^="A to"]')!;
		expect(same_name.textContent).not.toContain(f.origin.name);
		expect(same_name.querySelector('.distance')?.textContent).toMatch(/\d/);
		expect(same_name.getAttribute('aria-label')).not.toContain('undefined');
		expect(view.container.querySelectorAll('button[aria-label^="A to"]')).toHaveLength(1);
		expect(view.container.querySelector('button[aria-label^="D to"]')).toBeNull();
		await view.getByText('Show more', { exact: true }).click();
		await expect.element(view.getByRole('button', { name: /^D to D North.*3m/ })).toBeVisible();
		const panel = view.container.querySelector<HTMLElement>('section[aria-label="Nearby routes"]')!;
		expect(panel.clientHeight).toBeLessThanOrEqual(Math.min(220, window.innerHeight * 0.25));
		expect(panel.scrollHeight).toBeGreaterThan(panel.clientHeight);
		panel.scrollTop = panel.scrollHeight;
		await view.getByText('Show less', { exact: true }).click();
		expect(view.container.querySelector('button[aria-label^="D to"]')).toBeNull();
		await expect.element(view.getByText('Show more', { exact: true })).toBeVisible();
		expect(f.requests.filter((url) => url.includes('/service/'))).toHaveLength(1);
		const url = new URL(
			f.requests.find((url) => url.includes('/service/'))!,
			location.origin
		);
		expect(url.searchParams.get('route_ids')?.split(',').sort()).toEqual(['A', 'B', 'C', 'D']);
		view.container.style.width = '280px';
		const bounds = view.container.getBoundingClientRect();
		for (const button of view.container.querySelectorAll('button[aria-label*=" to "]')) {
			const box = button.getBoundingClientRect();
			expect(box.height).toBeGreaterThanOrEqual(44);
			expect(box.right).toBeLessThanOrEqual(bounds.right + 1);
		}
		await view.getByRole('button', { name: /^A to A North/ }).click();
		expect(page.state.modal).toMatchObject({ type: 'stop', source: 'mta_bus', id: 'far' });
		await view.getByText('Nearby routes', { exact: true }).click();
		await expect
			.element(view.container.querySelector<HTMLElement>('[aria-label="Nearby route options"]')!)
			.not.toBeVisible();
		await view.getByText('Nearby routes', { exact: true }).click();
		await expect.element(view.getByRole('button', { name: /^A to A North/ })).toBeVisible();
		expect(f.requests.filter((url) => url.includes('/service/'))).toHaveLength(1);
	} finally {
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});

test('fills the preview with predicted routes and removes routes with only passed or no calls', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	const f = fixtureData((f) => {
		f.service.routes.A = f.serviceFor('A', []);
		f.service.routes.B = f.serviceFor('B', [['far', -1, 0]]);
	});
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		child_props: { stop: f.origin }
	});
	try {
		await expect.element(view.getByRole('button', { name: /^C to C North.*8m/ })).toBeVisible();
		await expect.element(view.getByRole('button', { name: /^D to D North.*3m/ })).toBeVisible();
		expect(view.container.querySelector('button[aria-label^="A to"]')).toBeNull();
		expect(view.container.querySelector('button[aria-label^="B to"]')).toBeNull();
		await expect.element(view.getByText('Show more', { exact: true })).not.toBeInTheDocument();
	} finally {
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});

test('distinguishes loading, empty predictions and failures and recovers on retry', async () => {
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
	f.service_state.fail = true;
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data: f.data,
		component: Transfers,
		child_props: { stop: f.origin }
	});
	try {
		await expect.element(view.getByRole('status')).toHaveTextContent('Loading predictions…');
		expect(view.container.querySelector('button[aria-label*=" to "]')).toBeNull();
		release();
		await expect.poll(() => f.data.status.warning).toBe(true);
		await expect.element(view.getByRole('status')).not.toBeInTheDocument();
		f.service_state.fail = false;
		f.data.retry();
		await expect
			.element(view.getByRole('status'))
			.toHaveTextContent('No nearby predictions to show');
		f.service.routes.A = f.serviceFor('A', [['near', 7, 0]]);
		f.data.retry();
		await expect.element(view.getByRole('button', { name: /^A to A North.*7m/ })).toBeVisible();
		await expect.element(view.getByRole('status')).not.toBeInTheDocument();
	} finally {
		release();
		await view.unmount();
		f.data.destroy();
		vi.restoreAllMocks();
	}
});

test('a failed catalog remains a retryable transfer outcome before any options are known', async () => {
	vi.spyOn(Date, 'now').mockReturnValue(chartNow);
	page.data.selected_sources = ['mta_bus'];
	const f = nearbyFixture();
	let fail = true;
	const respond = fixtureFetcher({
		catalogs: { mta_bus: f.catalog },
		services: { mta_bus: f.service }
	});
	const data = new TransitData({
		sources: ['mta_bus'],
		batch_ms: 0,
		fetcher: async (input, init) => {
			if (fail && String(input).includes('/catalog/')) throw new Error('Unavailable');
			return respond(input, init);
		}
	});
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data,
		component: Transfers,
		child_props: { stop: f.origin }
	});
	try {
		await expect.element(view.getByRole('heading', { name: 'Something went wrong' })).toBeVisible();
		fail = false;
		await view.getByRole('button', { name: 'Try again' }).click();
		await expect.element(view.getByRole('button', { name: /^A to A North/ })).toBeVisible();
	} finally {
		await view.unmount();
		data.destroy();
		vi.restoreAllMocks();
	}
});
