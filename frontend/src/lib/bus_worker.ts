import type { BusStop } from '$lib/bus_api';

addEventListener('message', async (e) => {
	const { type, payload } = e.data;

	if (type === 'load') {
		// create search index
		// TODO: don't fetch stops twice
		const stops_res: BusStop[] = await (await fetch('/api/bus/stops')).json();

		// create_stops_index(stops_res);

		postMessage({ type: 'ready' });
	}
});
