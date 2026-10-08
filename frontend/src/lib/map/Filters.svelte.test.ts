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
