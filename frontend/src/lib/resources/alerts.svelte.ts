import { SvelteMap } from 'svelte/reactivity';

import icons from '$lib/icons';
import { LiveResource, createMultiSourceContext, source_info } from '$lib/resources/index.svelte';
import { current_time } from '$lib/util.svelte';

import type { ApiAlert, Source } from '@trainstatus/client';

export interface AlertResource {
	alerts: ApiAlert[];
	alerts_by_route: SvelteMap<string, ApiAlert[]>;
}

export function index_alerts(data: ApiAlert[]): AlertResource {
	// TODO: maybe combine express alerts here (i dont think there should ever be alerts specifically for express mta_subway tho)
	const alerts: ApiAlert[] = [];
	const alerts_by_route: SvelteMap<string, ApiAlert[]> = new SvelteMap();

	for (const alert of data) {
		const processed = {
			...alert,
			header_html: parse_html(alert.header_html),
			description_html: alert.description_html ? parse_html(alert.description_html) : undefined,
			start_time: new Date(alert.start_time),
			end_time: alert.end_time ? new Date(alert.end_time) : undefined,
			updated_at: new Date(alert.updated_at),
			created_at: new Date(alert.created_at)
		};

		alerts.push(processed);

		for (const entity of processed.entities) {
			if (!alerts_by_route.has(entity.route_id)) {
				alerts_by_route.set(entity.route_id, []);
			}
			alerts_by_route.get(entity.route_id)!.push(processed);
		}
	}

	return { alerts, alerts_by_route };
}

export function createAlertResource(source: Source, initial_value: AlertResource) {
	const resource = new LiveResource<AlertResource>(
		async (signal) => {
			console.log(`updating ${source} alerts`);

			const at = current_time.value;
			const query_params = at ? `?at=${at}` : '';
			const res = await fetch(`/api/v1/alerts/${source}${query_params}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error('Failed to fetch alerts');

			const data: ApiAlert[] = await res.json();

			return index_alerts(data);
		},
		{ initial_value, interval: source_info[source].refresh_interval.alerts, debounce: 500 }
	);

	let prev_time = current_time.value;
	$effect(() => {
		const val = current_time.value;
		if (val !== prev_time) {
			prev_time = val;
			resource.refresh();
		}
	});
	return resource;
}

export const alert_context =
	createMultiSourceContext<Record<Source, LiveResource<AlertResource>>>();

// TODO: maybe move parsing to backend and standardize icon format (which will be important if we have other sources)
const train_regex = /(\[(.+?)\])/gm;

function parse_html(html: string) {
	return html.replaceAll(train_regex, (_match, _p1, p2) => {
		const icon = icons.find((t) => t.name === p2) ?? icons[icons.length - 1];
		if (icon.complete_svg) return icon.svg;
		else
			return `<svg xmlns="http://www.w3.org/2000/svg" class="inline-block" width="1rem" height="1rem" viewBox="0 0 90 90" focusable="false"> ${icon.svg} </svg>`;
	});
}
