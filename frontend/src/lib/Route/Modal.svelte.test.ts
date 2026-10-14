import type { ComponentProps } from 'svelte';

import type { ApiAlert } from '#lib/client/index.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';
import { TransitData } from '#lib/transit/store.svelte.js';

import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Modal from './Modal.svelte';

test('asynchronously loaded and replaced alert cards are observed and update navigation', async () => {
	const observers: {
		targets: Set<Element>;
		notify: (target: Element) => void;
		disconnect: ReturnType<typeof vi.fn>;
	}[] = [];
	vi.stubGlobal(
		'IntersectionObserver',
		class {
			targets = new Set<Element>();
			disconnect = vi.fn(() => this.targets.clear());
			constructor(callback: IntersectionObserverCallback) {
				observers.push({
					targets: this.targets,
					notify: (target) =>
						callback(
							[{ target, isIntersecting: true } as IntersectionObserverEntry],
							this as unknown as IntersectionObserver
						),
					disconnect: this.disconnect
				});
			}
			observe(target: Element) {
				this.targets.add(target);
			}
		}
	);
	const responses: ((response: Response) => void)[] = [];
	const data = new TransitData({
		sources: ['mta_bus'],
		fetcher: (async (input) => {
			if (String(input).includes('/alerts/'))
				return new Promise<Response>((resolve) => responses.push(resolve));
			return new Response(JSON.stringify({ source: 'mta_bus', routes: [], stops: [] }));
		}) as typeof fetch
	});
	const alert = (id: string): ApiAlert => ({
		id,
		original_id: id,
		data: { source: 'mta_bus', alert_type: id, display_before_active: 0 },
		created_at: 0,
		updated_at: 100,
		start_time: 0,
		entities: [{ route_id: 'A', sort_order: 0 }],
		translations: [
			{
				section: 'header',
				format: 'html',
				language: 'en',
				text: `<a href="https://example.com">${id} text</a>`
			}
		]
	});
	const view = await render(TransitProvider<ComponentProps<typeof Modal>>, {
		props: {
			data,
			component: Modal,
			child_props: { source: 'mta_bus', route_id: 'A', time_format: 'time' }
		}
	});
	await expect.poll(() => responses.length).toBe(1);
	responses[0](new Response(JSON.stringify([alert('first'), alert('second')])));
	await expect.poll(() => view.container.querySelectorAll('.alert').length).toBe(2);
	const cards = Array.from(view.container.querySelectorAll('.alert'));
	await expect.poll(() => observers.filter((observer) => observer.targets.size).length).toBe(2);
	expect(observers.flatMap((observer) => [...observer.targets])).toEqual(cards);
	expect(view.container.querySelector('a')?.hasAttribute('href')).toBe(false);
	observers.find((observer) => observer.targets.has(cards[1]))!.notify(cards[1]);
	await expect.element(view.getByText('second', { exact: true })).toBeVisible();
	await expect.element(view.getByRole('button', { name: 'Next alert' })).toBeDisabled();
	data.retry();
	await expect.poll(() => responses.length).toBe(2);
	responses[1](new Response(JSON.stringify([alert('replacement')])));
	await expect.element(view.getByText('replacement', { exact: true })).toBeVisible();
	expect(observers.filter((observer) => observer.targets.size)).toHaveLength(1);
	expect(view.container.querySelector('a')?.hasAttribute('href')).toBe(false);

	data.retry();
	await expect.poll(() => responses.length).toBe(3);
	const refreshed = alert('replacement');
	refreshed.translations[0].text = '<a href="https://example.com/new">refreshed text</a>';
	responses[2](new Response(JSON.stringify([refreshed])));
	await expect.element(view.getByText('refreshed text', { exact: true })).toBeVisible();
	expect(view.container.querySelector('a')?.hasAttribute('href')).toBe(false);
	await view.unmount();
	expect(observers.every((observer) => observer.disconnect.mock.calls.length > 0)).toBe(true);
	data.destroy();
	vi.unstubAllGlobals();
});
