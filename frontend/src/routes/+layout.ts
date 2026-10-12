import { browser } from '$app/env';

import { layout_sources, sources_cookie } from '#lib/source_preferences.svelte.js';

import type { LayoutLoad } from './$types';

// Offline boots reuse the shell's build-time layout data, so the browser reads the cookie itself.
export const load: LayoutLoad = ({ data, url }) =>
	browser
		? {
				...data,
				selected_sources: layout_sources(
					sources_cookie(document.cookie),
					url.searchParams.get('src')
				),
				at: url.searchParams.get('at') ?? undefined
			}
		: data;
