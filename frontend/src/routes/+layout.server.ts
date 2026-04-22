import type { Source } from '$lib/client';
import { COOKIE_NAME, parse_sources, supported_sources } from '$lib/source_preferences.svelte';

import type { LayoutServerLoad } from './$types';

// lightweight server load that just reads the cookie and URL params and passes it to the universal load function.
// This way the client-side caching actually works properly since data isn't only being fetched server-side
export const load: LayoutServerLoad = async ({ url, cookies }) => {
	const cookie_value = cookies.get(COOKIE_NAME);
	let selected_sources = parse_sources(cookie_value);

	// Deep-link auto-enable: if URL has 'src' param, include it temporarily
	const src_param = url.searchParams.get('src');
	if (
		src_param &&
		supported_sources.includes(src_param as Source) &&
		!selected_sources.includes(src_param as Source)
	) {
		selected_sources = [...selected_sources, src_param as Source];
	}

	const at = url.searchParams.get('at') ?? undefined;

	return {
		selected_sources,
		at
	};
};
