import { create_stops_index, search_stops } from '$lib/search';

// interface LoadPayload {
// 	stops: Stop<'bus' | 'train'>[];
// }

// listen for messages
addEventListener('message', async (e) => {
	const { type, payload } = e.data;

	if (type === 'load') {
		create_stops_index(payload.bus_stops, payload.train_stops);
		// create search index
		// TODO: don't fetch stops twice
		// const stops_res: Stop[] = await (await fetch('/api/stops')).json();
		// const [stops_res, bus_stops_res] = await Promise.all([
		// 	fetch('/api/stops'),
		// 	fetch('/api/bus/stops')
		// ]);

		// const stops: Stop[] = await stops_res.json();
		// const bus_stops: BusStop[] = await bus_stops_res.json();

		// create_stops_index(stops);
		// create_bus_stops_index(bus_stops);

		postMessage({ type: 'ready' });
	}

	if (type === 'search') {
		// get search term
		const search_term = payload.search_term;
		const search_type = payload.search_type;
		// search posts index
		const results = await search_stops(search_term, search_type);
		// send message with results
		postMessage({ type: 'results', payload: { results: results, search_type } });
	}
});
