import { render } from 'svelte/server';

import { goto } from '$app/navigation';
import { page } from '$app/state';

import { expect, test, vi } from 'vitest';

import CurrentTimeHarness from './test/CurrentTimeHarness.svelte';
import { atParamToUnixSeconds, closeModal, createCurrentTime } from './url-params.svelte';

vi.mock('$app/state', () => ({
	page: { url: new URL('http://localhost/'), state: {} }
}));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

test('blank and non-numeric at params mean live', () => {
	expect(atParamToUnixSeconds(null)).toBeUndefined();
	expect(atParamToUnixSeconds('')).toBeUndefined();
	expect(atParamToUnixSeconds('   ')).toBeUndefined();
	expect(atParamToUnixSeconds('soon')).toBeUndefined();
	expect(atParamToUnixSeconds('1700000000')).toBe(1_700_000_000);
});

test('current time stores only finite unix seconds', () => {
	const currentTime = createCurrentTime();
	currentTime.value = Number.NaN;
	expect(currentTime.value).toBeUndefined();
	currentTime.value = 1_700_000_000;
	expect(currentTime.value).toBe(1_700_000_000);
	expect(currentTime.ms).toBe(1_700_000_000_000);
	currentTime.value = undefined;
	expect(currentTime.value).toBeUndefined();
});

test('layout times stay isolated and epoch zero remains historical', () => {
	const historical = createCurrentTime(0);
	const live = createCurrentTime();
	expect(historical.value).toBe(0);
	expect(historical.ms).toBe(0);
	expect(live.value).toBeUndefined();
	historical.value = 1700000000;
	expect(live.value).toBeUndefined();
	expect(atParamToUnixSeconds('0')).toBe(0);
});

test('a historical SSR tree cannot change a later live tree', async () => {
	const historical = await render(CurrentTimeHarness, { props: { at: 0 } });
	const live = await render(CurrentTimeHarness);
	expect(historical.body).toContain('historical:0:0');
	expect(live.body).toContain('live');
});

test('modal dismissal preserves epoch zero and respects time cleared before URL sync', () => {
	page.url = new URL('http://localhost/?s=stop&src=mta_bus&at=123');
	closeModal(0);
	expect(goto).toHaveBeenLastCalledWith('/?at=0', expect.any(Object));
	closeModal(undefined);
	expect(goto).toHaveBeenLastCalledWith('/', expect.any(Object));
});
