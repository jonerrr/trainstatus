import FlexSearch from 'flexsearch';
import type { Stop } from '$lib/api';

let stops_index: FlexSearch.Index;

export function create_stops_index(data: Stop[]) {
	// create the posts index
	stops_index = new FlexSearch.Index({ tokenize: 'forward' });
	data.forEach((stop) => {
		stops_index.add(stop.id, stop.name);
	});
}

export function searchPostsIndex(search_term: string) {
	if (search_term === '') {
		return null;
	}

	const results = stops_index.search(search_term);
	// console.log(results);
	if (results.length) {
		// Get first 12 results
		// TODO: calculate how many results can be on the screen
		return results.map((id) => id.toString()).slice(0, 12);
	}
}
