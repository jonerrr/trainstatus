import { type Writable } from 'svelte/store';
import { offline, current_time } from '$lib/stores';
import icons from '$lib/icons';

const train_regex = /(\[(.+?)\])/gm;
export function parse_html(html: string) {
	return html.replaceAll(train_regex, (_match, _p1, p2) => {
		const icon = icons.find((t) => t.name === p2) ?? icons[icons.length - 1];
		if (icon.complete_svg) return icon.svg;
		else
			return `<svg xmlsn="http://www.w3.org/2000/svg" class="inline-block" width="1rem" height="1rem" viewBox="0 0 90 90" focusable="false"> ${icon.svg} </svg>`;
	});
}

type Fetch = typeof fetch;

export async function update_data(
	fetch: Fetch,
	trip_store: Writable<Trip[]>,
	stop_time_store: Writable<StopTime[]>,
	alert_store: Writable<Alert[]>,
	time: number | null
) {
	try {
		const [tripsResponse, stopTimesResponse, alertsResponse] = await Promise.all([
			fetch(`/api/trips${time ? `?at=${time}` : ''}`),
			fetch(`/api/stops/times${time ? `?at=${time}` : ''}`),
			fetch(`/api/alerts${time ? `?at=${time}` : ''}`)
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
						created_at: new Date(t.created_at),
						updated_at: new Date(t.updated_at)
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
			alertsResponse.json().then((data: Alert[]) => {
				return data.map((a: Alert) => {
					return {
						...a,
						header_html: parse_html(a.header_html),
						description_html: a.description_html ? parse_html(a.description_html) : null,
						created_at: new Date(a.created_at),
						updated_at: new Date(a.updated_at),
						start_time: new Date(a.start_time),
						end_time: a.end_time ? new Date(a.end_time) : null
					};
				});
			})
		]);

		trip_store.set(trips);
		stop_time_store.set(stopTimes);
		alert_store.set(routeAlerts);
	} catch (e) {
		console.error(e);
		offline.set(true);
	}
}

export const all_route_ids = [
	'1',
	'2',
	'3',
	'4',
	'5',
	'6',
	'7',
	'A',
	'C',
	'E',
	'B',
	'D',
	'F',
	'M',
	'G',
	'J',
	'Z',
	'L',
	'N',
	'Q',
	'R',
	'W',
	'H',
	'FS',
	'GS',
	'SI'
];

////////////// stop stuff

export interface RouteStop {
	id: string;
	// I wonder if its possible to check the alerts and see if route is running on night service
	// also take into account rush hours https://new.mta.info/sites/default/files/2019-10/service_guide_web_Oct19.pdf
	stop_type: StopType;
	stop_sequence: number;
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
	transfers: string[];
}

////////////////// trip stuff

export interface Trip {
	id: string;
	route_id: string;
	express: boolean;
	direction: Direction;
	assigned: boolean;
	created_at: Date;
	stop_id: string | null;
	train_status: TrainStatus | null;
	current_stop_sequence: number | null;
	updated_at: Date;
}

export enum TrainStatus {
	Incoming = 0,
	AtStop = 1,
	InTransitTo = 2
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
// export interface RouteAlerts {
// 	route_id: string;
// 	alerts: Alert[];
// }
////////////////////// alert stuff

export interface Alert {
	id: string;
	alert_type: string;
	header_html: string;
	description_html: string | null;
	created_at: Date;
	updated_at: Date;
	start_time: Date;
	end_time: Date | null;
	entities: Entity[];
}

export interface Entity {
	alert_id: string;
	route_id: string | null;
	stop_id: string | null;
	sort_order: number;
}

// export interface ActivePeriod {
// 	start_time: Date;
// 	end_time: Date;
// }
