import { beforeEach, vi } from 'vitest';

import { reset } from './page.svelte';

vi.mock('$app/state', async () => {
	const mod = await import('./page.svelte');
	return { page: mod.page };
});

vi.mock('$app/navigation', async () => {
	const mod = await import('./page.svelte');
	return { goto: mod.goto };
});

beforeEach(() => {
	reset();
});
