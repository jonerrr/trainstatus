import { create_stops_index, searchPostsIndex } from '$lib/search';
import type { Stop } from '$lib/api';

// listen for messages
addEventListener('message', async (e) => {
	const { type, payload } = e.data;

	if (type === 'load') {
		// create search index

		const stops_res: Stop[] = await (await fetch('/api/stops')).json();

		create_stops_index(stops_res);
		// stops.subscribe((data) => {
		// 	console.log('creating index', data);
		// 	create_stops_index(data);
		// });

		postMessage({ type: 'ready' });
	}

	if (type === 'search') {
		// get search term
		const search_term = payload.search_term;
		// search posts index
		const results = searchPostsIndex(search_term);
		console.log(results);
		// send message with results
		postMessage({ type: 'results', payload: { results } });
	}
});
