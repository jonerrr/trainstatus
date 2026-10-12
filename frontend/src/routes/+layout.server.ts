import { building } from '$app/env';

import { COOKIE_NAME, default_sources, layout_sources } from '#lib/source_preferences.svelte.js';

import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ url, cookies }) => {
	// The prerendered offline shell has no request; the browser load restores preferences.
	if (building) return { selected_sources: default_sources, at: undefined };
	return {
		selected_sources: layout_sources(cookies.get(COOKIE_NAME), url.searchParams.get('src')),
		at: url.searchParams.get('at') ?? undefined
	};
};
