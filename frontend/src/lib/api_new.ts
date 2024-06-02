import { writable } from 'svelte/store';

export const stop_store = writable<Stop[]>([]);
export const trip_store = writable<Trip[]>([]);
export const stop_time_store = writable<StopTime[]>([]);

export async function init_stops() {
	try {
		const stops: Stop[] = await (await fetch('/api/stops')).json();
		stop_store.set(stops);

		const trips: Trip[] = (await (await fetch('/api/trips?times=false')).json()).map((t: Trip) => {
			return {
				...t,
				created_at: new Date(t.created_at)
			};
		});
		trip_store.set(trips);

		const stop_times: StopTime[] = (await (await fetch('/api/arrivals')).json()).map(
			(st: StopTime) => {
				return {
					...st,
					arrival: new Date(st.arrival),
					departure: new Date(st.departure)
				};
			}
		);
		stop_time_store.set(stop_times);
	} catch (e) {
		console.error(e);
	}
}

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

export interface Stop {
	id: string;
	name: string;
	ada: boolean;
	notes: string | null;
	borough: string;
	north_headsign: string;
	south_headsign: string;
	routes: RouteStop[];
}

// type TripStopTime = Omit<StopTime, 'assigned' | 'direction'>;

export interface Trip {
	id: string;
	route_id: string;
	direction: Direction;
	assigned: boolean;
	created_at: Date;
	stop_times: string[];
}

export enum Direction {
	North = 1,
	South = 0
}

export interface StopTime {
	stop_id: string;
	arrival: Date;
	departure: Date;
	direction: Direction;
	assigned: boolean;
	route_id: string;
	eta?: number;
	trip_id: string;
	// created_at: Date;
}
