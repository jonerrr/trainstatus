import { building } from '$app/env';

import { COOKIE_NAME, defaultSources, layoutSources } from '#lib/source-preferences.svelte.js';

import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ url, cookies }) => {
	// The prerendered offline shell has no request; the browser load restores preferences.
	if (building) return { selectedSources: defaultSources, at: undefined, offlineShell: true };
	return {
		selectedSources: layoutSources(cookies.get(COOKIE_NAME), url.searchParams.get('src')),
		at: url.searchParams.get('at') ?? undefined
	};
};
