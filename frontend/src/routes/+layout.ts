import type { Stop } from '$lib/api';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch }) => {
	const stops: Stop[] = await (await fetch('/api/stops')).json();

	return { stops };
};
