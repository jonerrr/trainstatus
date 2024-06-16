type Fetch = typeof fetch;

// monitored routes will change on geolocate, when searching, and clicking through dialogs
export async function update_data(fetch: Fetch) {
	//
}

export interface BusRoute {
	id: string;
	long_name: string;
	short_name: string;
	color: string;
	shuttle: boolean;
}

export interface BusStop {
	id: number;
	name: string;
	direction: string;
	lat: number;
	lon: number;
	routes: BusRouteStop[];
}

export interface BusRouteStop {
	id: string;
	direction: 0 | 1;
	headsign: string;
}

export interface BusStopTime {
	trip_id: string;
	stop_id: number;
	arrival: Date;
	departure: Date;
	stop_sequence: number;
}
