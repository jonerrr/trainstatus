import type { ComponentProps } from 'svelte';

import '../../app.css';

import { nearbyFixture } from '#lib/test/nearby-fixture.js';
import { page } from '#lib/test/page.svelte.js';
import { fixtureFetcher } from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';
import { TransitData } from '#lib/transit/store.svelte.js';

import { expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Transfers from './Transfers.svelte';

test('shows compact nearby choices without fetching predictions or repeating routes', async () => {
	const f = nearbyFixture();
	f.near.name = f.origin.name;
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
			return respond(input, init);
		}
	});
	data.replaceCatalog(f.catalog);
	const view = await render(TransitProvider<ComponentProps<typeof Transfers>>, {
		data,
		component: Transfers,
		child_props: { stop: f.origin }
	});
	try {
		await expect.element(view.getByRole('button', { name: /^A to A North/ })).toBeVisible();
		await expect.element(view.getByRole('button', { name: /^B to B North/ })).toBeVisible();
		const same_name = view.container.querySelector('button[aria-label^="A to"]')!;
		expect(same_name.textContent).not.toContain(f.origin.name);
		expect(same_name.querySelector('.metric')?.textContent).toMatch(/\d/);
		expect(view.container.querySelectorAll('button[aria-label^="A to"]')).toHaveLength(1);
		expect(view.container.textContent).not.toContain('D North');
		await view.getByRole('button', { name: 'Show all 4 options' }).click();
		await expect.element(view.getByRole('button', { name: /^D to D North/ })).toBeVisible();
		const options = view.container.querySelector<HTMLElement>(
			'[aria-label="Nearby route options"]'
		)!;
		expect(options.clientHeight).toBeLessThanOrEqual(Math.min(192, window.innerHeight * 0.25));
		expect(options.scrollHeight).toBeGreaterThan(options.clientHeight);
		options.scrollTop = options.scrollHeight;
		await expect.element(view.getByRole('button', { name: 'Show less' })).toBeVisible();
		await view.getByRole('button', { name: 'Show less' }).click();
		expect(view.container.textContent).not.toContain('D North');
		await expect.element(view.getByRole('button', { name: 'Show all 4 options' })).toBeVisible();
		expect(requests.filter((url) => url.includes('/service/'))).toEqual([]);
		view.container.style.width = '280px';
		const bounds = view.container.getBoundingClientRect();
		for (const button of view.container.querySelectorAll('button')) {
			const box = button.getBoundingClientRect();
			if (box.height === 0) continue;
			expect(box.height).toBeGreaterThanOrEqual(44);
			expect(box.right).toBeLessThanOrEqual(bounds.right + 1);
		}
		await view.getByRole('button', { name: /^A to A North/ }).click();
		expect(page.state.modal).toMatchObject({ type: 'stop', source: 'mta_bus', id: 'near' });
	} finally {
		await view.unmount();
		data.destroy();
	}
});
