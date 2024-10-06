import { create_stops_index, search_stops } from '$lib/search';

// interface LoadPayload {
// 	stops: Stop<'bus' | 'train'>[];
// }

// listen for messages
addEventListener('message', async (e) => {
	const { type, payload } = e.data;

	if (type === 'load') {
		create_stops_index(payload.bus_stops, payload.train_stops);

		postMessage({ type: 'ready' });
	}

	if (type === 'search') {
		// get search term
		const search_term = payload.search_term;
		const search_type = payload.search_type;

		const results = await search_stops(search_term, search_type);
		// send message with results
		postMessage({ type: 'results', payload: { results, search_type } });
	}
});
