import { init_data, type Stop } from '$lib/api';
import { stops, trips, stop_times, alerts } from '$lib/stores';
import type { LayoutLoad } from './$types';

// // load data here first for SSR benefits (i think)
// export const load: LayoutLoad = async ({ fetch }) => {
// 	// we only want to fetch stops once
// 	const stops_res: Stop[] = await (await fetch('/api/stops')).json();
// 	stops.set(stops_res);

// 	await init_data(fetch, trips, stop_times, alerts);

// 	// return { ok: 'ok' };
// };

export const load: LayoutLoad = async ({ fetch }) => {
	const stopsPromise = fetch('/api/stops').then((res) => res.json());
	const initDataPromise = init_data(fetch, trips, stop_times, alerts);

	const [stops_res] = await Promise.all([stopsPromise, initDataPromise]);

	stops.set(stops_res);
};
