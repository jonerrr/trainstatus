import {
	create_stops_index,
	create_bus_stops_index,
	search_stops,
	search_bus_stops
} from '$lib/search';
import type { Stop } from '$lib/api';
import type { BusStop } from '$lib/bus_api';

// listen for messages
addEventListener('message', async (e) => {
	const { type, payload } = e.data;

	if (type === 'load') {
		// create search index
		// TODO: don't fetch stops twice
		// const stops_res: Stop[] = await (await fetch('/api/stops')).json();
		const [stops_res, bus_stops_res] = await Promise.all([
			fetch('/api/stops'),
			fetch('/api/bus/stops')
		]);

		const stops: Stop[] = await stops_res.json();
		const bus_stops: BusStop[] = await bus_stops_res.json();

		create_stops_index(stops);
		create_bus_stops_index(bus_stops);

		postMessage({ type: 'ready' });
	}

	if (type === 'search') {
		// get search term
		const search_term = payload.search_term;
		const search_type = payload.search_type;
		// search posts index
		const results =
			search_type === 'Train' ? search_stops(search_term) : search_bus_stops(search_term);
		// send message with results
		postMessage({ type: 'results', payload: { results: results ?? [], search_type } });
	}
});
