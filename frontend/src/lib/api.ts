import { offline } from '$lib/stores';
import icons from '$lib/icons';

export interface Stop {
	id: string;
	name: string;
	ada: boolean;
	notes?: string;
	borough: string;
	north_headsign: string;
	south_headsign: string;
	routes: Route[];
}

export interface Route {
	id: string;
	stop_type: StopType;
}

export enum StopType {
	FullTime = 0,
	PartTime = 1,
	LateNights = 2,
	RushHourOneDirection = 3,
	RushHourExtension = 4
}

type Fetch = typeof fetch;

export async function fetch_stops(fetch: Fetch): Promise<Stop[]> {
	const res = await fetch('/api/stops');
	// check if response is from service worker
	offline.set(res.headers.has('x-service-worker'));

	const data: Stop[] = await res.json();

	return data;
}

export interface Trip {
	id: string;
	route_id: string;
	direction: Direction;
	assigned: boolean;
	created_at: string;
	stop_times: StopTime[];
	eta?: number;
}

export enum Direction {
	North = 1,
	South = 0
}

export interface StopTime {
	stop_id: string;
	arrival: string;
	departure: string;
}

export async function fetch_trips(fetch: Fetch, stops: string[]): Promise<Trip[]> {
	const res = await fetch(`/api/trips?stop_ids=${stops.join(',')}`);
	// check if response is from service worker
	offline.set(res.headers.has('x-service-worker'));

	const data: Trip[] = await res.json();

	return data;
}

export interface RouteAlerts {
	route_id: string;
	alerts: Alert[];
}

export interface Alert {
	alert_type: string;
	header: string;
	description?: string;
	updated_at: string;
	active_periods: ActivePeriod[];
}

export interface ActivePeriod {
	start_time: string;
	end_time: string;
}

const train_regex = /(\[(.+?)\])/gm;
function parse_html(html: string) {
	return html.replaceAll(train_regex, (_match, _p1, p2) => {
		const icon = icons.find((t) => t.name === p2) ?? icons[icons.length - 1];
		// TODO: check if full svg and add missing shuttle bus icon
		if (icon.complete_svg) return icon.svg;
		else
			return `<svg xmlsn="http://www.w3.org/2000/svg" class="inline-block" width="1rem" height="1rem" viewBox="0 0 90 90" focusable="false"> ${icon.svg} </svg>`;
	});
}

export async function fetch_alerts(fetch: Fetch, routes: string[]): Promise<RouteAlerts[]> {
	const res = await fetch(`/api/alerts?route_ids=${routes.join(',')}`);
	// check if response is from service worker
	offline.set(res.headers.has('x-service-worker'));

	const data: RouteAlerts[] = await res.json();

	for (const route of data) {
		for (const alert of route.alerts) {
			alert.header = parse_html(alert.header);
			if (alert.description) alert.description = parse_html(alert.description);
		}
	}

	return data;
}
