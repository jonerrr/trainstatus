import { fetch_stops } from '$lib/api';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch }) => {
	// const stops = await fetch_stops(fetch);

	return { stops: await fetch_stops(fetch) };
};
