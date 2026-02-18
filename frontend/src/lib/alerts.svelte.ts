import { SvelteMap } from 'svelte/reactivity';

import icons from '$lib/icons';
import { LiveResource } from '$lib/rt-resource.svelte';
import { source_info } from '$lib/sources';

import type { ApiAlert, Source } from '@trainstatus/client';
import { Context, resource } from 'runed';

interface AlertResource {
	alerts: ApiAlert[];
	alerts_by_route: SvelteMap<string, ApiAlert[]>;
}

export function createAlertResource(source: Source, params: { at?: number }) {
	// const sourceDeps = () =>
	// 	({
	// 		at: params.at
	// 	}) satisfies { at?: number };

	const resource = new LiveResource<AlertResource>(
		async (signal) => {
			console.log('updating alerts');
			const query = new URLSearchParams();
			if (params.at) query.set('at', params.at.toString());

			const res = await fetch(`/api/v1/alerts/${source}?${query}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error('Failed to fetch alerts');

			const data: ApiAlert[] = await res.json();

			const alerts = data.map((alert) => ({
				...alert,
				header_html: parse_html(alert.header_html),
				description_html: alert.description_html ? parse_html(alert.description_html) : undefined,
				start_time: new Date(alert.start_time),
				end_time: alert.end_time ? new Date(alert.end_time) : undefined,
				updated_at: new Date(alert.updated_at),
				created_at: new Date(alert.created_at)
			}));

			// alerts_by_route.clear();
			// might not need to be a sveltemap
			const alerts_by_route: SvelteMap<string, ApiAlert[]> = new SvelteMap();

			// alerts_by_route = new SvelteMap<string, Alert[]>();
			for (const alert of alerts) {
				for (const entity of alert.entities) {
					if (!alerts_by_route.has(entity.route_id)) {
						alerts_by_route.set(entity.route_id, []);
					}
					// TODO: fix date types being strings
					alerts_by_route.get(entity.route_id)!.push(alert);
				}
			}

			return {
				alerts,
				alerts_by_route
			};
		},
		{ interval: source_info[source].refresh_interval, debounce: 500 }
	);

	$effect(() => {
		if (params.at !== undefined) {
			resource.refresh();
		}
	});
	return resource;
	// const alertResource = resource(
	// 	() => params.at,
	// 	async (at, prevAt, { signal }): Promise<AlertResource> => {
	// 		const query = new URLSearchParams();
	// 		if (at) query.set('at', at.toString());

	// 		const res = await fetch(`/api/v1/alerts/${source}?${query}`, { signal });

	// 		if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
	// 		if (!res.ok) throw new Error('Failed to fetch alerts');

	// 		const data: ApiAlert[] = await res.json();

	// 		const alerts = data.map((alert) => ({
	// 			...alert,
	// 			header_html: parse_html(alert.header_html),
	// 			description_html: alert.description_html ? parse_html(alert.description_html) : undefined,
	// 			start_time: new Date(alert.start_time),
	// 			end_time: alert.end_time ? new Date(alert.end_time) : undefined,
	// 			updated_at: new Date(alert.updated_at),
	// 			created_at: new Date(alert.created_at)
	// 		}));

	// 		// alerts_by_route.clear();
	// 		// might not need to be a sveltemap
	// 		const alerts_by_route: SvelteMap<string, ApiAlert[]> = new SvelteMap();

	// 		// alerts_by_route = new SvelteMap<string, Alert[]>();
	// 		for (const alert of alerts) {
	// 			for (const entity of alert.entities) {
	// 				if (!alerts_by_route.has(entity.route_id)) {
	// 					alerts_by_route.set(entity.route_id, []);
	// 				}
	// 				// TODO: fix date types being strings
	// 				alerts_by_route.get(entity.route_id)!.push(alert);
	// 			}
	// 		}

	// 		return {
	// 			alerts,
	// 			alerts_by_route
	// 		};
	// 	},
	// 	{
	// 		initialValue: {
	// 			alerts: [],
	// 			alerts_by_route: new SvelteMap()
	// 		},
	// 		debounce: 500 // TODO: maybe do debounce instead or increase time
	// 	}
	// );

	// return alertResource;
}

export const alert_context = new Context<ReturnType<typeof createAlertResource>>('alerts');

export interface Alert {
	id: string;
	alert_type: string;
	header_html: string;
	description_html?: string;
	start_time: Date;
	end_time?: Date;
	updated_at: Date;
	created_at: Date;
	entities: Entity[];
}

export interface Entity {
	route_id: string;
	sort_order: number;
	stop_id?: number;
}

type Fetch = typeof fetch;

// TODO: maybe move parsing to backend and standardize icon format (which will be important if we have other sources)
const train_regex = /(\[(.+?)\])/gm;

function parse_html(html: string) {
	return html.replaceAll(train_regex, (_match, _p1, p2) => {
		const icon = icons.find((t) => t.name === p2) ?? icons[icons.length - 1];
		if (icon.complete_svg) return icon.svg;
		else
			return `<svg xmlsn="http://www.w3.org/2000/svg" class="inline-block" width="1rem" height="1rem" viewBox="0 0 90 90" focusable="false"> ${icon.svg} </svg>`;
	});
}

export function createAlerts() {
	let alerts: Alert[] = $state([]);
	const alerts_by_route: SvelteMap<string, Alert[]> = $state(new SvelteMap());

	async function update(fetch: Fetch, at?: string) {
		// const res = await fetch(`/api/v1/alerts${at ? `?at=${at}` : ''}`);
		// if (res.headers.has('x-sw-fallback')) {
		// 	throw new Error('Offline');
		// }
		// const data: Alert[] = await res.json();
		// alerts = data.map((alert) => ({
		// 	...alert,
		// 	header_html: parse_html(alert.header_html),
		// 	description_html: alert.description_html ? parse_html(alert.description_html) : undefined,
		// 	start_time: new Date(alert.start_time),
		// 	end_time: alert.end_time ? new Date(alert.end_time) : undefined,
		// 	updated_at: new Date(alert.updated_at),
		// 	created_at: new Date(alert.created_at)
		// }));
		// alerts_by_route.clear();
		// // alerts_by_route = new SvelteMap<string, Alert[]>();
		// for (const alert of alerts) {
		// 	for (const entity of alert.entities) {
		// 		if (!alerts_by_route.has(entity.route_id)) {
		// 			alerts_by_route.set(entity.route_id, []);
		// 		}
		// 		alerts_by_route.get(entity.route_id)!.push(alert);
		// 	}
		// }
	}

	return {
		update,

		get alerts() {
			return alerts;
		},

		get alerts_by_route() {
			return alerts_by_route;
		}
	};
}

export const alerts = createAlerts();
