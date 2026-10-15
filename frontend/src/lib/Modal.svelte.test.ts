import { flushSync } from 'svelte';

import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import { chartFixture } from './charts/fixtures';
import Modal from './Modal.svelte';
import ModalFocusHarness from './test/ModalFocusHarness.svelte';
import ModalHarness from './test/ModalHarness.svelte';
import { page } from './test/page.svelte';
import { fixtureFetcher } from './test/transit-fixture';
import TransitDataHarness from './test/TransitDataHarness.svelte';
import TransitProvider from './test/TransitProvider.svelte';
import { TransitData } from './transit/store.svelte';

function state(id: string, index: number): App.PageState {
	return {
		index,
		modal: { type: 'route', source: 'njt_bus', id }
	};
}

async function settle() {
	await new Promise<void>((resolve) =>
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
	);
	await expect.poll(() => document.getAnimations().length).toBe(0);
}

afterEach(async () => {
	await settle();
	vi.restoreAllMocks();
});

test('opening from home makes the modal visible', async () => {
	page.state = { modal: null };
	await render(ModalHarness);
	flushSync(() => {
		page.state = state('First', 1);
	});
	await settle();
	expect(
		document.querySelector('[role="dialog"], dialog')?.getBoundingClientRect().height
	).toBeGreaterThan(0);
});

test('two system backs preserve the outgoing sheet until the return transition', async () => {
	page.state = state('Third', 3);
	await render(ModalHarness);
	const dialog = document.querySelector<HTMLElement>('[role="dialog"], dialog')!;
	const snapshots: Array<{ open: boolean; direction?: string }> = [];
	const original = document.startViewTransition.bind(document);
	vi.spyOn(document, 'startViewTransition').mockImplementation((update) => {
		snapshots.push({
			open: dialog.getBoundingClientRect().height > 0,
			direction: document.documentElement.dataset.modalDirection
		});
		return original(update);
	});
	let index = 3;
	vi.spyOn(history, 'back').mockImplementation(() => {
		setTimeout(() => {
			page.state = state(index === 2 ? 'Second' : 'First', index);
		}, 0);
	});
	// Replay the recorded Android behavior even on engines supporting closedby:
	// one cancelable close, then an unconditional close without a new activation.
	for (const cancelable of [true, false]) {
		index--;
		if (dialog instanceof HTMLDialogElement && dialog.open) {
			const event = new Event('cancel', { cancelable });
			dialog.dispatchEvent(event);
			if (!event.defaultPrevented) dialog.close();
		} else {
			history.back();
		}
		// No frame may expose the page or a reopened sheet between stacked modals.
		expect(dialog.getBoundingClientRect().height).toBeGreaterThan(0);
		await settle();
	}
	expect(snapshots).toEqual([
		{ open: true, direction: 'backward' },
		{ open: true, direction: 'backward' }
	]);
	page.state = { modal: null, index: 0 };
	await settle();
	expect(dialog.getBoundingClientRect().height).toBe(0);
	page.state = state('First', 1);
	await settle();
	expect(dialog.textContent).toContain('First');
	expect(snapshots.slice(2)).toEqual([
		{ open: true, direction: 'backward' },
		{ open: false, direction: 'forward' }
	]);
});

test('Firefox system back falls through to history instead of a disabled close watcher', async () => {
	page.state = state('Second', 2);
	await render(ModalHarness);
	// Firefox registers watchers for every open native dialog, including
	// closedby=none. Its Android host consumes back even if that watcher is disabled.
	if (!document.querySelector('dialog[open]')) {
		page.state = state('First', 1);
	}
	await settle();
	expect(document.querySelector('[role="dialog"], dialog')?.textContent).toContain('First');
});

test('keeps focus inside the sheet and restores the page after Escape', async () => {
	page.state = { modal: null };
	await render(ModalHarness);
	const background = document.querySelector<HTMLElement>('[data-background]')!;
	background.focus();
	flushSync(() => {
		page.state = state('First', 1);
	});
	await settle();
	const sheet = document.querySelector<HTMLElement>('[role="dialog"]')!;
	const buttons = sheet.querySelectorAll('button');
	expect(background.inert).toBe(true);
	expect(sheet.contains(document.activeElement)).toBe(true);
	expect(document.body.style.overflow).toBe('hidden');
	buttons[0].focus();
	await userEvent.keyboard('{Shift>}{Tab}{/Shift}');
	expect(document.activeElement).toBe(buttons[buttons.length - 1]);
	await userEvent.keyboard('{Tab}');
	expect(document.activeElement).toBe(buttons[0]);
	await userEvent.keyboard('{Escape}');
	await settle();
	expect(sheet.getBoundingClientRect().height).toBe(0);
	expect(background.inert).toBe(false);
	expect(document.activeElement).toBe(background);
	expect(document.body.style.overflow).toBe('');
});

test('backdrop drags keep the sheet open and a backdrop tap dismisses it', async () => {
	page.state = state('First', 1);
	await render(ModalHarness);
	const backdrop = document.querySelector<HTMLElement>('.modal-backdrop')!;
	backdrop.dispatchEvent(
		new PointerEvent('pointerdown', { clientX: 1, clientY: 1, bubbles: true })
	);
	backdrop.dispatchEvent(new PointerEvent('pointerup', { clientX: 40, clientY: 1, bubbles: true }));
	await settle();
	expect(page.state.modal?.id).toBe('First');
	backdrop.dispatchEvent(
		new PointerEvent('pointerdown', { clientX: 1, clientY: 1, bubbles: true })
	);
	backdrop.dispatchEvent(new PointerEvent('pointercancel', { bubbles: true }));
	backdrop.dispatchEvent(new PointerEvent('pointerup', { clientX: 1, clientY: 1, bubbles: true }));
	await settle();
	expect(page.state.modal?.id).toBe('First');
	backdrop.dispatchEvent(
		new PointerEvent('pointerdown', { clientX: 1, clientY: 1, bubbles: true })
	);
	backdrop.dispatchEvent(new PointerEvent('pointerup', { clientX: 1, clientY: 1, bubbles: true }));
	await settle();
	expect(document.querySelector('[role="dialog"]')?.getBoundingClientRect().height).toBe(0);
});

test('a rapid tap on the sheet during navigation does not dismiss the modal', async () => {
	page.state = state('First', 1);
	await render(ModalHarness);
	await settle();
	const original = document.startViewTransition.bind(document);
	let transition: ViewTransition | undefined;
	vi.spyOn(document, 'startViewTransition').mockImplementation((update) => {
		transition = original(update);
		return transition;
	});
	flushSync(() => {
		page.state = state('Second', 2);
	});
	await transition!.ready;
	const sheet = document.querySelector<HTMLElement>('[role="dialog"]')!;
	const box = sheet.getBoundingClientRect();
	const clientX = box.x + box.width / 2;
	const clientY = box.y + box.height / 2;
	const target = document.elementFromPoint(clientX, clientY)!;
	for (const type of ['pointerdown', 'pointerup']) {
		target.dispatchEvent(
			new PointerEvent(type, { clientX, clientY, pointerType: 'touch', bubbles: true })
		);
	}
	await settle();
	expect(page.state.modal?.id).toBe('Second');
	expect(sheet.getBoundingClientRect().height).toBeGreaterThan(0);
});

test('includes trip disclosure summaries when wrapping keyboard focus', async () => {
	await render(ModalFocusHarness);
	const summary = document.querySelector('summary')!;
	const button = document.querySelector('button')!;
	button.focus();
	await userEvent.keyboard('{Tab}');
	expect(document.activeElement).toBe(summary);
	await userEvent.keyboard('{Shift>}{Tab}{/Shift}');
	expect(document.activeElement).toBe(button);
});

for (const { age, present } of [
	{ age: 10_000, present: true },
	{ age: 31_000, present: true },
	{ age: 31_000, present: false }
]) {
	test(`a trip missing from ${age}ms cached service waits for refreshed membership (${present})`, async () => {
		const fixture = chartFixture('njt_bus', 0);
		const empty = {
			...fixture.service,
			routes: {
				'4': {
					trips: [],
					stops: [],
					calls: { trip: [], stop: [], sequence: [], arrival: [], departure: [] }
				}
			}
		};
		const initial = fixtureFetcher({
			catalogs: { njt_bus: fixture.catalog },
			services: { njt_bus: empty }
		});
		const refreshed = fixtureFetcher({
			catalogs: { njt_bus: fixture.catalog },
			services: { njt_bus: present ? fixture.service : empty }
		});
		let clock = 1_780_000_000_000;
		vi.spyOn(Date, 'now').mockImplementation(() => clock);
		let requests = 0;
		let release!: () => void;
		const gate = new Promise<void>((resolve) => (release = resolve));
		const fetcher = (async (input: RequestInfo | URL, init?: RequestInit) => {
			if (String(input).includes('/service/')) {
				if (++requests > 1) {
					await gate;
					return refreshed(input, init);
				}
			}
			return initial(input, init);
		}) as typeof fetch;
		const data = new TransitData({ sources: ['njt_bus'], fetcher, batch_ms: 0 });
		try {
			const reader = await render(TransitDataHarness, {
				data,
				source: 'njt_bus',
				readers: [{ id: 'old', route: '4' }]
			});
			await expect
				.poll(() => data.loadedStopArrivals('njt_bus', fixture.catalog.stops[0].id).routes.has('4'))
				.toBe(true);
			await reader.unmount();
			clock += age;
			page.data.selected_sources = ['njt_bus'];
			page.state = { modal: { type: 'trip', source: 'njt_bus', route_id: '4', id: 'trip-0' } };
			const view = await render(TransitProvider<Record<string, never>>, { data, component: Modal });
			await expect.poll(() => requests).toBe(2);
			expect(page.state.modal?.id).toBe('trip-0');
			release();
			if (present) {
				await expect.poll(() => data.loadedTrip('njt_bus', '4', 'trip-0')?.trip.id).toBe('trip-0');
				await expect.element(view.getByRole('dialog')).toBeVisible();
				expect(page.state.modal?.id).toBe('trip-0');
			} else {
				await expect.poll(() => page.state.modal ?? null).toBeNull();
			}
			await settle();
			expect(requests).toBe(2);
			await view.unmount();
		} finally {
			release();
			data.destroy();
		}
	});
}
