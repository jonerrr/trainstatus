import Dexie, { type EntityTable } from 'dexie';
// import type { Route } from '$lib/api';

// TODO: some timestamp thing to delete old stuff
interface RouteStop {
	id: string;
	// maybe store name idk yet or just join
	// name: string;
	// I wonder if its possible to check the alerts and see if route is running on night service
	// also take into account rush hours https://new.mta.info/sites/default/files/2019-10/service_guide_web_Oct19.pdf
	stop_type: StopType;
	// arrivals: number[];
}

enum StopType {
	FullTime = 0,
	PartTime = 1,
	LateNights = 2,
	RushHourOneDirection = 3,
	RushHourExtension = 4
}

interface Stop {
	id: string;
	name: string;
	ada: boolean;
	notes?: string;
	borough: string;
	north_headsign: string;
	south_headsign: string;
	// reference to route id
	route_ids: string[];
	// routes: Route[];
	// reference to trips
	trip_ids: string[];
}

type TripStopTime = Omit<StopTime, 'assigned' | 'direction'>;

interface Trip {
	id: string;
	route_id: string;
	direction: Direction;
	assigned: boolean;
	created_at: Date;
	stop_times: string[];
	// stop_times: TripStopTime[];

	// eta?: number;
}

enum Direction {
	North = 1,
	South = 0
}

interface StopTime {
	stop_id: string;
	arrival: Date;
	departure: Date;
	direction: Direction;
	assigned: boolean;
	// created_at: Date;
}

const db = new Dexie('StopsDatabase') as Dexie & {
	stop: EntityTable<
		Stop,
		'id' // primary key "id" (for the typings only)
	>;
	route_stop: EntityTable<RouteStop, 'id'>;
	trip: EntityTable<Trip, 'id'>;
	stop_time: EntityTable<StopTime, 'stop_id'>;
};

db.version(1).stores({
	stop: '&id,&name,ada,notes,borough,north_headsign,south_headsign,route_ids,*trip_ids',
	route_stop: '&id,stop_type',
	trip: '&id,route_id,direction,assigned,created_at,*stop_times',
	stop_time: '&stop_id,arrival,departure,direction,assigned'
});

export type { RouteStop, Stop, Trip, StopTime, StopType, Direction };
export { db };

// https://github.com/dexie/Dexie.js/issues/281 for stop name search
