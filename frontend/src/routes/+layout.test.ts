import { afterEach, expect, test, vi } from 'vitest';

import { load } from './+layout';
import type { LayoutLoad } from './$types';

vi.mock('$app/env', () => ({ browser: true, building: false }));

afterEach(() => vi.unstubAllGlobals());

function read(data: Parameters<LayoutLoad>[0]['data'], search = '') {
	return load({
		data,
		url: new URL(`https://trainstat.us/stops${search}`)
	} as Parameters<LayoutLoad>[0]);
}

test('normal browser loads use the server-parsed preferences and time', () => {
	const selected: Parameters<LayoutLoad>[0]['data'] = { selectedSources: ['mta_bus'], at: '123' };
	// A normal load can use server data without reading document.cookie.
	expect(read(selected)).toBe(selected);
});

test('offline shell data restores the saved sources, deep link source and zero time', () => {
	vi.stubGlobal('document', { cookie: 'selectedSources=mta_subway' });
	expect(
		read({ selectedSources: ['mta_bus'], offlineShell: true, at: undefined }, '?src=njt_bus&at=0')
	).toEqual({
		selectedSources: ['mta_subway', 'njt_bus'],
		offlineShell: true,
		at: '0'
	});
});
