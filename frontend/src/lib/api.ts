import FlexSearch from 'flexsearch';
import { writable } from 'svelte/store';
import { offline } from '$lib/stores';
import icons from '$lib/icons';

export const trip_store = writable<Trip[]>([]);
export const stop_time_store = writable<StopTime[]>([]);
export const route_alerts_store = writable<RouteAlerts[]>([]);

export let stopsIndex: FlexSearch.Index;

const train_regex = /(\[(.+?)\])/gm;
function parse_html(html: string) {
	return html.replaceAll(train_regex, (_match, _p1, p2) => {
		const icon = icons.find((t) => t.name === p2) ?? icons[icons.length - 1];
		if (icon.complete_svg) return icon.svg;
		else
			return `<svg xmlsn="http://www.w3.org/2000/svg" class="inline-block" width="1rem" height="1rem" viewBox="0 0 90 90" focusable="false"> ${icon.svg} </svg>`;
	});
}

export async function init_data() {
	try {
		const [tripsResponse, stopTimesResponse, alertsResponse] = await Promise.all([
			fetch('/api/trips?times=false'),
			fetch('/api/arrivals'),
			fetch('/api/alerts')
		]);

		if (
			tripsResponse.headers.has('x-service-worker') ||
			stopTimesResponse.headers.has('x-service-worker') ||
			alertsResponse.headers.has('x-service-worker')
		)
			offline.set(true);
		else offline.set(false);

		const [trips, stopTimes, routeAlerts] = await Promise.all([
			tripsResponse.json().then((data: Trip[]) => {
				return data.map((t: Trip) => {
					return {
						...t,
						created_at: new Date(t.created_at)
					};
				});
			}),
			stopTimesResponse.json().then((data: StopTime[]) => {
				return data.map((st: StopTime) => {
					return {
						...st,
						arrival: new Date(st.arrival),
						departure: new Date(st.departure)
					};
				});
			}),
			alertsResponse.json().then((data: RouteAlerts[]) => {
				return data.map((ra: RouteAlerts) => {
					return {
						...ra,
						alerts: ra.alerts.map((a: Alert) => {
							return {
								...a,
								header: parse_html(a.header),
								description: a.description ? parse_html(a.description) : null,
								updated_at: new Date(a.updated_at)
							};
						})
					};
				});
			})
		]);

		trip_store.set(trips);
		stop_time_store.set(stopTimes);
		route_alerts_store.set(routeAlerts);
	} catch (e) {
		console.error(e);
	}
}

export interface RouteStop {
	id: string;
	// maybe store name idk yet or just join
	// name: string;
	// I wonder if its possible to check the alerts and see if route is running on night service
	// also take into account rush hours https://new.mta.info/sites/default/files/2019-10/service_guide_web_Oct19.pdf
	stop_type: StopType;
	// arrivals: number[];
}

export enum StopType {
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
	lat: number;
	lon: number;
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

// Alert types
export interface RouteAlerts {
	route_id: string;
	alerts: Alert[];
}

export interface Alert {
	alert_type: string;
	header: string;
	description: string | null;
	updated_at: Date;
	active_periods: ActivePeriod[];
}

export interface ActivePeriod {
	start_time: Date;
	end_time: Date;
}
