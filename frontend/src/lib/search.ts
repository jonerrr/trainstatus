import FlexSearch from 'flexsearch';
import type { Stop } from '$lib/static';

let train_stops_index: FlexSearch.Index;
let bus_stops_index: FlexSearch.Index;

let train_stops: Stop<'train'>[];
let bus_stops: Stop<'bus'>[];

export function create_stops_index(bus_data: Stop<'bus'>[], train_data: Stop<'train'>[]) {
	// create the posts index
	train_stops_index = new FlexSearch.Index({ tokenize: 'forward' });
	bus_stops_index = new FlexSearch.Index({ tokenize: 'forward' });
	// data.forEach((stop, i) => {
	// 	if (stop.type === 'bus') {
	// 		bus_stops_index.add(i, stop.name);
	// 		bus_stops.push(stop as Stop<'bus'>);
	// 	} else {
	// 		train_stops_index.add(i, stop.name);
	// 		train_stops.push(stop as Stop<'train'>);
	// 	}
	// });
	train_data.forEach((stop, i) => {
		train_stops_index.add(i, stop.name);
		// train_stops.push(stop);
	});
	train_stops = train_data;
	bus_data.forEach((stop, i) => {
		bus_stops_index.add(i, stop.name);
		// bus_stops.push(stop);
	});
	bus_stops = bus_data;
}

export async function search_stops(search_term: string, type: 'bus' | 'train') {
	if (search_term === '') {
		return null;
	}

	const results =
		type === 'bus'
			? await bus_stops_index.searchAsync(search_term, { limit: 15 })
			: await train_stops_index.searchAsync(search_term, { limit: 15 });

	return results.map((i) => (type === 'bus' ? bus_stops[i as number] : train_stops[i as number]));
}
