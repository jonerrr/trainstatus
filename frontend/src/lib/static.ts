export interface Route {
	id: string;
	long_name: string;
	short_name: string;
	color: string;
	shuttle: boolean;
	route_type: RouteType;
	// geom
}

export enum RouteType {
	Train,
	Bus
}

type StopMapping = {
	bus: {
		data: BusStopData;
		routes: BusRouteStop;
	};
	train: {
		data: TrainStopData;
		routes: TrainRouteStop;
	};
};

// Update the Stop interface to use the mapped type
export interface Stop<T extends keyof StopMapping> {
	id: number;
	name: string;
	lat: number;
	lon: number;
	data: StopMapping[T]['data'];
	routes: StopMapping[T]['routes'][];
	type: T;
}

// export interface Stop<D extends StopData, R extends RouteStop> {
// 	id: number;
// 	name: string;
// 	lat: number;
// 	lon: number;
// 	data: D;
// 	routes: R;
// }

// type StopData = TrainStopData | BusStopData;

export interface TrainStopData {
	ada: boolean;
	north_headsign: string;
	south_headsign: string;
	transfers: number[];
	notes: string | null;
	// borough: string
}

export interface BusStopData {
	direction: BusStopDirection;
}

export enum BusStopDirection {
	SW = 'SW',
	S = 'S',
	SE = 'SE',
	E = 'E',
	W = 'W',
	NW = 'NW',
	NE = 'NE',
	N = 'N',
	Blank = ''
}

// type RouteStop = BusRouteStop | TrainRouteStop;

export interface BusRouteStop {
	id: number;
	stop_sequence: number;
	headsign: string;
	direction: 0 | 1;
}

export interface TrainRouteStop {
	id: string;
	stop_sequence: number;
	type: StopType | null;
}

export enum StopType {
	RushHour = 'rush_hour',
	LateNight = 'late_night',
	FullTime = 'full_time',
	PartTime = 'part_time'
}