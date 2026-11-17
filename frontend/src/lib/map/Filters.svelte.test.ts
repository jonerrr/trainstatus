import { page } from '#lib/test/page.svelte.js';

import { expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import Filters from './Filters.svelte';
import { MapFilters } from './filters.svelte';

test('filters leave the map available and stay open until explicitly dismissed', async () => {
	const { getByRole } = await render(Filters, { filters: new MapFilters() });
	await getByRole('button', { name: 'Filters', exact: true }).click();
	const panel = document.querySelector('#map-filter-panel')!;
	expect(panel.getAttribute('aria-modal')).not.toBe('true');
	expect(document.querySelector('.filter-backdrop')).toBeNull();
	document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
	expect(document.querySelector('#map-filter-panel')).not.toBeNull();
	await userEvent.keyboard('{Escape}');
	expect(document.querySelector('#map-filter-panel')).toBeNull();
});

test('source filters follow layout availability while preserving hidden choices', async () => {
	page.data.selectedSources = ['mta_subway', 'mta_bus'];
	const filters = new MapFilters();
	const view = await render(Filters, { filters });
	await view.getByRole('button', { name: 'Filters', exact: true }).click();
	filters.toggleSource('mta_bus');
	expect(filters.sources).toEqual(['mta_subway']);

	page.data.selectedSources = ['mta_bus', 'njt_bus'];
	await expect.poll(() => filters.sources).toEqual(['njt_bus']);
	expect(filters.isSourceEnabled('mta_bus')).toBe(false);
	expect(filters.activeFilterCount).toBe(1);
	filters.reset();
	await expect.poll(() => filters.sources).toEqual(['mta_bus', 'njt_bus']);
	expect(filters.activeFilterCount).toBe(0);
	await view.unmount();
});
