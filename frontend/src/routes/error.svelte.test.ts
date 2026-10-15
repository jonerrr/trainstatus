import { SvelteURL } from 'svelte/reactivity';

import { goto } from '$app/navigation';

import { page } from '#lib/test/page.svelte.js';

import { beforeEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import ErrorPage from './+error.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

beforeEach(() => {
	vi.mocked(goto).mockReset().mockResolvedValue(undefined);
});

test('the error prop renders without page.error and retry preserves the shared URL and modal', async () => {
	page.url = new SvelteURL('http://localhost/?s=S1&src=mta_bus&at=123');
	page.state = { modal: { type: 'stop', source: 'mta_bus', id: 'S1' } };
	const view = await render(ErrorPage, { error: { status: 500, message: 'Please try again.' } });
	await expect.element(view.getByRole('heading', { name: 'Something went wrong' })).toBeVisible();
	await expect.element(view.getByText('Please try again.')).toBeVisible();
	await view.getByRole('button', { name: 'Try again' }).click();
	expect(goto).toHaveBeenCalledWith(page.url.href, {
		replace: true,
		refreshAll: true,
		state: page.state
	});
	await expect
		.element(view.getByRole('link', { name: 'Reload page' }))
		.toHaveAttribute('href', page.url.href);
	await expect
		.element(view.getByRole('link', { name: 'Reload page' }))
		.toHaveAttribute('data-sveltekit-reload');
});

test('a rejected retry stays recoverable instead of throwing from the click handler', async () => {
	vi.mocked(goto).mockRejectedValue(new Error('Navigation failed'));
	const view = await render(ErrorPage, { error: { status: 500, message: 'Please try again.' } });
	await view.getByRole('button', { name: 'Try again' }).click();
	await expect
		.element(view.getByRole('alert'))
		.toHaveTextContent('Couldn’t reopen this page. Try reloading it.');
	await expect.element(view.getByRole('button', { name: 'Try again' })).toBeEnabled();
});

test('a missing route offers a home link and renders the framework message', async () => {
	const view = await render(ErrorPage, { error: { status: 404, message: 'Not Found' } });
	await expect.element(view.getByRole('heading', { name: 'Page not found' })).toBeVisible();
	await expect.element(view.getByText('Not Found', { exact: true })).toBeVisible();
	await expect.element(view.getByRole('link', { name: 'Go home' })).toHaveAttribute('href', '/');
	await expect.element(view.getByRole('button', { name: 'Try again' })).not.toBeInTheDocument();
});
