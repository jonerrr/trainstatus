import { expect, test, vi } from 'vitest';

import { atParamToUnixSeconds, current_time } from './url_params.svelte';

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
	current_time.value = Number.NaN;
	expect(current_time.value).toBeUndefined();
	current_time.value = 1_700_000_000;
	expect(current_time.value).toBe(1_700_000_000);
	expect(current_time.ms).toBe(1_700_000_000_000);
	current_time.value = undefined;
	expect(current_time.value).toBeUndefined();
});
