import { SvelteMap } from 'svelte/reactivity';

import type { ApiAlert, Source } from '$lib/client';
import icons from '$lib/icons';
import {
	type AlertResource,
	type AlertResources,
	LiveResource,
	type TypedAlert,
	createMultiSourceContext,
	source_info
} from '$lib/resources/index.svelte';
import { current_time } from '$lib/url_params.svelte';

export function index_alerts<S extends Source>(data: ApiAlert[]): AlertResource<S> {
	// TODO: maybe combine express alerts here (i dont think there should ever be alerts specifically for express mta_subway tho)
	const alerts: TypedAlert<S>[] = [];
	const alerts_by_route: SvelteMap<string, TypedAlert<S>[]> = new SvelteMap();

	for (const alert of data) {
		// const header = alert.translations.find((t) => t.section === 'header')?.text ?? '';
		// const description = alert.translations.find((t) => t.section === 'description')?.text;

		const processed = {
			...alert,
			translations: alert.translations.map((t) => ({
				...t,
				// TODO: only use this for mta_subway or standardize icons and stuff across sources
				text: t.format === 'html' ? parse_html(t.text) : t.text
			})),
			start_time: new Date(alert.start_time),
			end_time: alert.end_time ? new Date(alert.end_time) : undefined,
			updated_at: new Date(alert.updated_at),
			created_at: new Date(alert.created_at)
		} as TypedAlert<S>;

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

export function createAlertResource<S extends Source>(source: S, initial_value: AlertResource<S>) {
	const resource = new LiveResource<AlertResource<S>>(
		async (signal) => {
			console.log(`updating ${source} alerts`);

			const at = current_time.value;
			const query_params = at ? `?at=${at}` : '';
			const res = await fetch(`/api/v1/alerts/${source}${query_params}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error('Failed to fetch alerts');

			const data: ApiAlert[] = await res.json();

			return index_alerts<S>(data);
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

export const alert_context = createMultiSourceContext<AlertResources>();

// TODO: maybe move parsing to backend and standardize icon format (which will be important if we have other sources)
const mta_subway_icon_regex = /(\[(.+?)\])/gm;

function parse_html(html: string) {
	return html.replaceAll(mta_subway_icon_regex, (_match, _p1, p2) => {
		const icon = icons.find((t) => t.name === p2) ?? icons[icons.length - 1];
		if (icon.complete_svg) return icon.svg;
		else
			return `<svg xmlns="http://www.w3.org/2000/svg" class="inline-block" width="1rem" height="1rem" viewBox="0 0 90 90" focusable="false"> ${icon.svg} </svg>`;
	});
}
