import { LocalStorage } from '#lib/storage.svelte.js';
import StorageHarness from '#lib/test/StorageHarness.svelte';

import { afterEach, expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';

const key = 'storage-test';
afterEach(() => localStorage.removeItem(key));

test('saved values stay stable and changes update other instances and persist', async () => {
	localStorage.setItem(key, JSON.stringify('saved'));
	const first = new LocalStorage(key, 'default');
	const second = new LocalStorage(key, 'default');
	const view = await render(StorageHarness, { value: first });
	await expect.element(view.getByText('saved', { exact: true })).toBeVisible();
	await view.getByRole('button', { name: 'Change preference' }).click();
	await expect.element(view.getByText('changed', { exact: true })).toBeVisible();
	expect(second.current).toBe('changed');
	expect(JSON.parse(localStorage.getItem(key)!)).toBe('changed');
	await view.unmount();
});

test('cross-tab replacement, deletion and malformed values recover without overwriting storage', async () => {
	const value = new LocalStorage(key, 'default');
	const view = await render(StorageHarness, { value });
	const changed = () =>
		window.dispatchEvent(new StorageEvent('storage', { key, storageArea: localStorage }));
	localStorage.setItem(key, JSON.stringify('external'));
	changed();
	await expect.element(view.getByText('external', { exact: true })).toBeVisible();
	localStorage.removeItem(key);
	changed();
	await expect.element(view.getByText('default', { exact: true })).toBeVisible();
	localStorage.setItem(key, 'malformed');
	changed();
	expect(value.current).toBe('default');
	expect(localStorage.getItem(key)).toBe('malformed');
	await view.unmount();
});
