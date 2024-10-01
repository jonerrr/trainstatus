import type { Route, Stop } from '$lib/static';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch }) => {
	// add stop.type of 'bus' | 'train' to Stop
	const stops_promise = fetch('/api/stops').then((res) => res.json());
	const routes_promise = fetch('/api/routes').then((res) => res.json());

	const [stops, routes]: [Stop<'bus' | 'train'>[], Route[]] = await Promise.all([
		stops_promise,
		routes_promise
	]);

	return { stops, routes };
};
