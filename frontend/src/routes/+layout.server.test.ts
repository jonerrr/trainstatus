import { describe, expect, it } from 'vitest';

import { load } from './+layout.server';
import type { LayoutServerLoad } from './$types';

function loadFrom(search: string, cookie?: string) {
	const url = new URL(`https://trainstat.us/${search}`);
	const event = {
		url,
		cookies: {
			get: (name: string) => (name === 'selected_sources' ? cookie : undefined)
		},
		parent: async () => ({}),
		depends: () => {},
		untrack: (fn) => fn()
	} as Parameters<LayoutServerLoad>[0];
	return load(event);
}

describe('layout source deep link', () => {
	it('leaves the cookie list when src is missing', async () => {
		await expect(loadFrom('', 'mta_subway')).resolves.toMatchObject({
			selected_sources: ['mta_subway'],
			at: undefined
		});
	});

	it('ignores an invalid src', async () => {
		await expect(loadFrom('?src=not-a-source', 'mta_subway')).resolves.toMatchObject({
			selected_sources: ['mta_subway']
		});
	});

	it('does not duplicate an already selected src', async () => {
		await expect(loadFrom('?src=mta_subway', 'mta_subway,mta_bus')).resolves.toMatchObject({
			selected_sources: ['mta_subway', 'mta_bus']
		});
	});

	it('appends a new valid src and passes at through', async () => {
		await expect(loadFrom('?src=njt_bus&at=123', 'mta_subway')).resolves.toEqual({
			selected_sources: ['mta_subway', 'njt_bus'],
			at: '123'
		});
	});
});
