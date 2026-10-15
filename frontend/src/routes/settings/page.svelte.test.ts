import { SvelteURL } from 'svelte/reactivity';

import { page } from '#lib/test/page.svelte.js';
import SettingsHarness from '#lib/test/SettingsHarness.svelte';

import { expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';

test('editing time waits for Apply, and returning to live clears the URL time', async () => {
	page.url = new SvelteURL('http://localhost/settings?keep=value');
	const view = await render(SettingsHarness);
	const field = view.getByLabelText('Historical time');
	await field.fill('2026-10-01T12:00');
	expect(page.url.searchParams.has('at')).toBe(false);
	await expect.element(view.getByText('live', { exact: true })).toBeVisible();
	await view.getByRole('button', { name: 'Apply', exact: true }).click();
	await expect.poll(() => page.url.searchParams.has('at')).toBe(true);
	const at = page.url.searchParams.get('at');
	await expect.element(view.getByText(`historical:${at}:${Number(at) * 1000}`)).toBeVisible();
	expect(page.url.searchParams.get('keep')).toBe('value');
	await field.fill('2026-10-02T12:00');
	expect(page.url.searchParams.get('at')).toBe(at);
	await view.getByRole('button', { name: 'Return to live', exact: true }).click();
	await expect.poll(() => page.url.searchParams.has('at')).toBe(false);
	await expect.element(view.getByText('live', { exact: true })).toBeVisible();
	await expect.element(field).toHaveValue('');
	await view.unmount();
});
