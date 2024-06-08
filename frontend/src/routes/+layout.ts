import { init_data, parse_html, type Alert, type Stop, type StopTime, type Trip } from '$lib/api';
import { stops, trips, stop_times, alerts } from '$lib/stores';
import type { LayoutLoad } from './$types';

// load data here first for SSR benefits (i think)
export const load: LayoutLoad = async ({ fetch }) => {
	// we only want to fetch stops once
	const stops_res: Stop[] = await (await fetch('/api/stops')).json();
	stops.set(stops_res);

	await init_data(fetch, trips, stop_times, alerts);

	// return { ok: 'ok' };
};
