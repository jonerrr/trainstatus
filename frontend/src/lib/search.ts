import FlexSearch from 'flexsearch';
import type { Stop } from '$lib/api';
import type { BusStop } from '$lib/bus_api';

let stops_index: FlexSearch.Index;

export function create_stops_index(data: Stop[]) {
	// create the posts index
	stops_index = new FlexSearch.Index({ tokenize: 'forward' });
	data.forEach((stop) => {
		stops_index.add(stop.id, stop.name);
	});
}

export function search_stops(search_term: string) {
	if (search_term === '') {
		return null;
	}

	const results = stops_index.search(search_term);
	if (results.length) {
		// Get first 12 results
		// TODO: calculate how many results can be on the screen
		return results.map((id) => id.toString()).slice(0, 12);
	}
}

let bus_stops_index: FlexSearch.Index;

export function create_bus_stops_index(data: BusStop[]) {
	// create the posts index
	bus_stops_index = new FlexSearch.Index({ tokenize: 'forward' });
	data.forEach((stop) => {
		bus_stops_index.add(stop.id, stop.name);
	});
}

export function search_bus_stops(search_term: string) {
	if (search_term === '') {
		return null;
	}

	const results = bus_stops_index.search(search_term);
	if (results.length) {
		// Get first 12 results
		// TODO: calculate how many results can be on the screen
		return results
			.map((id) => id.toString())
			.slice(0, 12)
			.map((id) => parseInt(id));
	}
}
