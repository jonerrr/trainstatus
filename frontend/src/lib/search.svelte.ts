import type { Stop } from '$lib/static';

import { Index } from 'flexsearch';

interface Indexes {
	train: Index;
	bus: Index;
}

interface Stops {
	train: Stop<'train'>[];
	bus: Stop<'bus'>[];
}

export class StopSearch {
	indexes = $state({} as Indexes);
	stops = $state({} as Stops);

	constructor(bus_data: Stop<'bus'>[], train_data: Stop<'train'>[]) {
		this.indexes['train'] = new Index({ tokenize: 'forward' });
		this.indexes['bus'] = new Index({ tokenize: 'forward' });

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
		const results = this.indexes[type].search(search_term);

		return results.map((i) => this.stops[type][i as number]);
	}
}
