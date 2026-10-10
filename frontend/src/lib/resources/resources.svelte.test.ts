import { flushSync } from 'svelte';

import { expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page as browserPage } from 'vitest/browser';

import type { Stop } from '../client';
import { watchTrajectories } from '../map/trajectories';
import { trajectoryFixture } from '../map/trajectoryFixture';
import { page } from '../test/page.svelte';
import type { ResourceControls } from '../test/resourceControls';
import ResourceHarness from '../test/ResourceHarness.svelte';

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((yes) => {
		resolve = yes;
	});
	return { promise, resolve };
}
test('trajectory decode failures use the shared indicator and Retry, and unregister on disposal', async () => {
	let controls!: ResourceControls;
	const view = await render(ResourceHarness, {
		fetcher: async () => [],
		routeFetcher: async () => new Response('[]'),
		onready: (next) => {
			controls = next;
		}
	});
	await expect.poll(() => controls.resource.available).toBe(true);
	let response = Promise.resolve(new Response('invalid Arrow'));
	const stop = watchTrajectories(
		{ sources: ['mta_bus'], at: 0, refreshInterval: 0 },
		() => {},
		() => response,
		controls.status
	);
	const indicator = view.getByRole('button', { name: /Retry updates/ });
	await expect.element(indicator).toBeVisible();
	const retry = deferred<Response>();
	response = retry.promise;
	await indicator.click();
	expect(controls.status.failed).toBe(true);
	retry.resolve(new Response(trajectoryFixture()));
	await expect.element(indicator).not.toBeInTheDocument();
	response = Promise.resolve(new Response('', { status: 503 }));
	await stop.refresh().catch(() => {});
	await expect.element(indicator).toBeVisible();
	stop();
	await expect.element(indicator).not.toBeInTheDocument();
	await view.unmount();
	expect(controls.status.entries.size).toBe(0);
});
test.each(['completed', 'pending', 'failed'])(
	'scrolling between bus stops on the same routes does not restart a %s request',
	async (state) => {
		page.data.selected_sources = ['mta_bus'];
		page.data.routes_by_id = {
			mta_bus: {
				A: {
					id: 'A',
					short_name: 'A',
					long_name: 'Route A',
					color: '',
					text_color: '',
					data: {
						source: 'mta_bus',
						name_number: 1,
						name_prefix: 'A',
						service_types: [],
						sort_key: 1
					}
				}
			}
		};
		const stops: Stop[] = Array.from({ length: 100 }, (_, i) => ({
			id: String(i),
			name: `Stop ${i}`,
			geom: { Point: { x: 0, y: 0 } },
			transfers: [],
			data: { source: 'mta_bus', direction: 'n', is_boardable: true },
			routes: [
				{
					route_id: 'A',
					stop_id: String(i),
					stop_sequence: i,
					data: { source: 'mta_bus', direction: 0, headsign: 'Destination' }
				}
			]
		}));
		const calls: string[] = [];
		const work = deferred<Response>();
		let signal: AbortSignal | undefined;
		let controls!: ResourceControls;
		const view = await render(ResourceHarness, {
			stops,
			fetcher: async () => [],
			routeFetcher: async (url, options) => {
				calls.push(String(url));
				signal = options?.signal ?? undefined;
				if (state === 'failed') throw new Error('Offline');
				return state === 'pending' ? work.promise : new Response('[]');
			},
			onready: (next) => {
				controls = next;
			}
		});
		await expect.poll(() => calls.length).toBe(1);
		const viewport = view.container.querySelector<HTMLDivElement>('.overflow-y-auto')!;
		for (const top of [2500, 5000, 7500]) {
			viewport.scrollTop = top;
			viewport.dispatchEvent(new Event('scroll'));
			await new Promise((resolve) => setTimeout(resolve, 650));
		}
		expect(view.container.textContent).toContain('Stop 75');
		expect(view.container.textContent).not.toContain('Stop 0');
		expect(signal?.aborted).toBe(false);
		await view.unmount();
		expect(calls).toEqual(['/api/v1/stop_times/mta_bus?route_ids=A']);
		expect(controls.arrivals.active).toBe(false);
		if (state === 'pending') expect(signal?.aborted).toBe(true);
	}
);
test('the Svelte owner publishes only the latest time and stops work on unmount', async () => {
	let controls!: ResourceControls;
	const calls: Array<{
		at: number | null;
		signal: AbortSignal;
		work: ReturnType<typeof deferred<string[]>>;
	}> = [];
	const view = await render(ResourceHarness, {
		fetcher: (at, signal) => {
			const work = deferred<string[]>();
			calls.push({ at, signal, work });
			return work.promise;
		},
		routeFetcher: async () => new Response('[]'),
		onready: (next) => {
			controls = next;
		}
	});
	await expect.poll(() => calls.length).toBe(1);
	const waiting = controls.resource.whenAvailable().catch((error: Error) => error.name);
	flushSync(() => {
		controls.time.value = 1;
	});
	flushSync(() => {
		controls.time.value = 2;
	});
	await expect.poll(() => calls.length).toBe(2);
	expect(calls[0].signal.aborted).toBe(true);
	expect(await waiting).toBe('AbortError');
	calls[0].work.resolve(['old']);
	calls[1].work.resolve(['new']);
	await expect.poll(() => view.container.querySelector('[data-values]')?.textContent).toBe('new');
	const pending = controls.resource.refresh().catch((error: Error) => error.name);
	await expect.poll(() => calls.length).toBe(3);
	await view.unmount();
	expect(await pending).toBe('AbortError');
	expect(calls[2].signal.aborted).toBe(true);
	calls[2].work.resolve(['late']);
	expect(controls.status.entries.size).toBe(0);
});
test('route holders are reference counted and availability belongs to the covering query', async () => {
	let controls!: ResourceControls;
	const calls: Array<{ url: string; work: ReturnType<typeof deferred<Response>> }> = [];
	const view = await render(ResourceHarness, {
		fetcher: async () => [],
		routeFetcher: async (url) => {
			const work = deferred<Response>();
			calls.push({ url: String(url), work });
			return work.promise;
		},
		onready: (next) => {
			controls = next;
		}
	});
	expect(calls).toHaveLength(0);
	const first = controls.arrivals.add_route('A');
	const second = controls.arrivals.add_route('A');
	await expect.poll(() => calls.length).toBe(1);
	const b = controls.arrivals.add_route('B');
	calls[0].work.resolve(new Response('[]'));
	await expect.poll(() => calls.length).toBe(2);
	expect(controls.arrivals.coversRoute('B')).toBe(false);
	calls[1].work.resolve(new Response('[]'));
	expect((await first).by_stop_id.size).toBe(0);
	await second;
	await b;
	controls.arrivals.remove_route('A');
	expect(controls.arrivals.coversRoute('A')).toBe(true);
	controls.arrivals.remove_route('A');
	expect(controls.arrivals.coversRoute('A')).toBe(false);
	controls.arrivals.remove_route('B');
	expect(controls.arrivals.active).toBe(false);
	await view.unmount();
});
test('failed updates retain data and retry inside the mobile navbar without covering content', async () => {
	await browserPage.viewport(390, 844);
	let controls!: ResourceControls;
	let response = Promise.resolve<string[]>(['saved']);
	const view = await render(ResourceHarness, {
		fetcher: () => response,
		routeFetcher: async () => new Response('[]'),
		onready: (next) => {
			controls = next;
		}
	});
	await expect.poll(() => controls.resource.available).toBe(true);
	const content = view.container.querySelector('[data-content]')!.getBoundingClientRect();
	const nav = view.container.querySelector('nav')!.getBoundingClientRect();
	response = Promise.reject(new Error('Connection failed'));
	response.catch(() => {});
	await controls.resource.refresh(true).catch(() => {});
	const indicator = view.getByRole('button', { name: /Retry updates/ });
	await expect.element(indicator).toBeVisible();
	expect(view.container.querySelector('[data-values]')?.textContent).toBe('saved');
	expect(view.container.querySelector('[data-content]')!.getBoundingClientRect().top).toBe(
		content.top
	);
	expect(view.container.querySelector('nav')!.getBoundingClientRect().top).toBe(nav.top);

	// The slide wrapper clips the button while its visible height animates.
	const bar = view.container
		.querySelector('.update-control')!
		.parentElement!.getBoundingClientRect();
	expect(bar.top).toBeGreaterThanOrEqual(nav.top);
	expect(bar.bottom).toBeLessThanOrEqual(nav.bottom);
	const home = view.container.querySelector('a[aria-label="Home"]')!.getBoundingClientRect();
	expect(home.top).toBeGreaterThanOrEqual(bar.bottom);
	expect(home.bottom).toBeLessThanOrEqual(nav.bottom);
	await expect.element(view.getByRole('dialog')).not.toBeInTheDocument();
	const retry = deferred<string[]>();
	response = retry.promise;
	await indicator.click();
	await expect.poll(() => controls.resource.fetching).toBe(true);
	await expect.element(indicator).toBeVisible();
	await expect.element(view.getByRole('dialog')).not.toBeInTheDocument();
	retry.resolve(['restored']);
	await expect.element(view.getByRole('button', { name: /Retry updates/ })).not.toBeInTheDocument();
});
test('the single failure flag stays true until every affected request recovers', async () => {
	let controls!: ResourceControls;
	let tripsFail = true;
	let arrivalsFail = true;
	const view = await render(ResourceHarness, {
		fetcher: async () => {
			if (tripsFail) throw new Error('Failed update');
			return ['saved'];
		},
		routeFetcher: async () => new Response('[]', { status: arrivalsFail ? 503 : 200 }),
		onready: (next) => {
			controls = next;
		}
	});
	await expect.poll(() => controls.status.failed).toBe(true);
	await controls.arrivals.add_route('A').catch(() => {});
	tripsFail = false;
	await controls.resource.refresh(true);
	expect(controls.status.failed).toBe(true);
	await expect
		.element(view.getByRole('button', { name: /Updates unavailable. Retry updates/ }))
		.toBeVisible();
	arrivalsFail = false;
	await controls.arrivals.refresh(true);
	expect(controls.status.failed).toBe(false);
	await expect
		.element(view.getByRole('button', { name: /Updates unavailable. Retry updates/ }))
		.not.toBeInTheDocument();
	await view.unmount();
});
