import { fetch_stops } from '$lib/api';
import type { Stop } from '$lib/api_new';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch }) => {
	// const stops = await fetch_stops(fetch);

	const stops: Stop[] = await (await fetch('/api/stops')).json();

	return { stops };
};
