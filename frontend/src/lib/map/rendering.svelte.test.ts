import '../../app.css';

import { expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { commands, page, userEvent } from 'vitest/browser';

import MapHarness from '../test/MapHarness.svelte';

test('real WebGL picking survives overlay removal and reattachment', async () => {
	await page.viewport(1100, 800);
	const view = await render(MapHarness);
	await expect.poll(() => view.component.read().map?.loaded()).toBe(true);
	const pick = () =>
		view.component
			.read()
			.picker?.pick({ x: 400, y: 250 })
			.map((v) => v.tripId) ?? [];
	await expect.poll(pick).toContain('trip');
	await view.getByRole('button', { name: 'Toggle vehicles' }).click();
	expect(view.component.read().picker).toBeNull();
	await view.getByRole('button', { name: 'Toggle vehicles' }).click();
	await expect.poll(pick).toContain('trip');
	const map = view.component.read().map!;
	await view.unmount();
	expect(map.getCanvas().isConnected).toBe(false);
});

test('filters and chooser allow the initiating drag to move the real map', async () => {
	await page.viewport(1100, 800);
	const view = await render(MapHarness);
	await expect.poll(() => view.component.read().map?.loaded()).toBe(true);
	await view.getByRole('button', { name: 'Filters', exact: true }).click();
	const before = view.component.read().map!.getCenter().lng;
	await commands.dragMap();
	await expect.poll(() => view.component.read().map!.getCenter().lng).not.toBe(before);
	expect(document.querySelector('#map-filter-panel')).not.toBeNull();
	await view.getByRole('button', { name: 'Choose features' }).click();
	const next = view.component.read().map!.getCenter().lng;
	await commands.dragMap();
	await expect.poll(() => view.component.read().map!.getCenter().lng).not.toBe(next);
	expect(document.querySelector('.chooser-panel')).toBeNull();
});

test('chooser keyboard dismissal restores its trigger without trapping focus', async () => {
	await page.viewport(1100, 800);
	const view = await render(MapHarness);
	await view.getByRole('button', { name: 'Choose features' }).click();
	await expect.element(view.getByRole('dialog', { name: 'Choose a map feature' })).toBeVisible();
	const target = document.querySelector<HTMLButtonElement>('[data-map-target]')!;
	expect(document.activeElement).toBe(target);
	await userEvent.keyboard('{Escape}');
	expect(document.querySelector('.chooser-panel')).toBeNull();
	expect(document.activeElement?.textContent).toBe('Choose features');
});
