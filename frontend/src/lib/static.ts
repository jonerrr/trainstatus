export interface Route {
	id: string;
	long_name: string;
	short_name: string;
	color: string;
	shuttle: boolean;
	route_type: 'bus' | 'train';
	// geom
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
	route_type: T;
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

export type BusStopDirection = 'sw' | 's' | 'se' | 'e' | 'w' | 'ne' | 'nw' | 'n' | 'unknown';

// type RouteStop = BusRouteStop | TrainRouteStop;

export interface BusRouteStop {
	id: string;
	stop_sequence: number;
	headsign: string;
	direction: 0 | 1;
}

export interface TrainRouteStop {
	id: string;
	stop_sequence: number;
	type: 'rush_hour' | 'late_night' | 'full_time' | 'part_time' | null;
}

// export enum StopType {
// 	RushHour = 'rush_hour',
// 	LateNight = 'late_night',
// 	FullTime = 'full_time',
// 	PartTime = 'part_time'
// }

export const is_bus = (s: Stop<'bus' | 'train'>): s is Stop<'bus'> => {
	return (s as Stop<'bus'>).route_type === 'bus';
};

export const is_train = (s: Stop<'bus' | 'train'>): s is Stop<'train'> => {
	return (s as Stop<'train'>).route_type === 'train';
};

// these stop types should be shown
export const always_stop = ['full_time', 'part_time'];
// get the main stop routes for a stop
export const main_stop_routes = (stop: Stop<'bus' | 'train'>) => {
	return stop.routes.filter(
		(r) => !is_train(stop) || always_stop.includes((r as TrainRouteStop).type!)
	);
};

export const calculate_route_height = () => 52;

export function calculate_stop_height(item: Stop<'bus'> | Stop<'train'>) {
	let height = 44; // stop name height (28px) + 16px padding
	if (is_bus(item)) {
		height += item.routes.length * 56;
	} else {
		// headsign height
		height += 24;
		// route arrivals height
		height += item.routes.length * 24;
	}

	return height;
	// each bus route has height of 80
}
