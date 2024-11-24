import FlexSearch from 'flexsearch';
import type { Stop } from '$lib/static';

interface Indexes {
	train: FlexSearch.Index;
	bus: FlexSearch.Index;
}

interface Stops {
	train: Stop<'train'>[];
	bus: Stop<'bus'>[];
}

export class StopSearch {
	indexes = $state(<Indexes>{});
	stops = $state(<Stops>{});

	constructor(bus_data: Stop<'bus'>[], train_data: Stop<'train'>[]) {
		this.indexes['train'] = new FlexSearch.Index({ tokenize: 'forward' });
		this.indexes['bus'] = new FlexSearch.Index({ tokenize: 'forward' });

		train_data.forEach((stop, i) => {
			this.indexes['train'].add(i, stop.name);
		});

		bus_data.forEach((stop, i) => {
			this.indexes['bus'].add(i, stop.name);
		});

		this.stops.train = train_data;
		this.stops.bus = bus_data;
	}

	search(search_term: string, type: 'bus' | 'train') {
		const results = this.indexes[type].search(search_term, 15);

		return results.map((i) => this.stops[type][i as number]);
	}
}
