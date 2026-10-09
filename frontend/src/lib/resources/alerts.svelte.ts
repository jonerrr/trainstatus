import { SvelteDate, SvelteMap, SvelteSet } from 'svelte/reactivity';

import type { ApiAlert, Source } from '#lib/client/index.js';
import icons from '#lib/icons.js';
import {
	createMultiSourceContext,
	LiveResource,
	source_info,
	type AlertResource,
	type AlertResources,
	type TypedAlert
} from '#lib/resources/index.svelte.js';
import { getCurrentTime } from '#lib/url_params.svelte.js';

export function index_alerts<S extends Source>(data: ApiAlert[]): AlertResource<S> {
	// TODO: maybe combine express alerts here (i dont think there should ever be alerts specifically for express mta_subway tho)
	const alerts: TypedAlert<S>[] = [];
	const alerts_by_route: SvelteMap<string, TypedAlert<S>[]> = new SvelteMap();

	for (const alert of data) {
		// const header = alert.translations.find((t) => t.section === 'header')?.text ?? '';
		// const description = alert.translations.find((t) => t.section === 'description')?.text;

		const typed_alert = alert as TypedAlert<S>;
		const processed: TypedAlert<S> = {
			...typed_alert,
			translations: alert.translations.map((t) => ({
				...t,
				// TODO: only use this for mta_subway or standardize icons and stuff across sources
				text: t.format === 'html' ? parse_html(t.text) : t.text
			})),
			start_time: new SvelteDate(alert.start_time),
			end_time: alert.end_time ? new SvelteDate(alert.end_time) : undefined,
			updated_at: new SvelteDate(alert.updated_at),
			created_at: new SvelteDate(alert.created_at)
		};

		alerts.push(processed);

		// An alert can affect multiple stops on a route, but belongs in its route list only once.
		for (const route_id of new SvelteSet(processed.entities.map((entity) => entity.route_id))) {
			if (!alerts_by_route.has(route_id)) {
				alerts_by_route.set(route_id, []);
			}
			alerts_by_route.get(route_id)!.push(processed);
		}
	}

	return { alerts, alerts_by_route };
}

export function createAlertResource<S extends Source>(source: S) {
	const current_time = getCurrentTime();
	const resource = new LiveResource<AlertResource<S>>(
		async (signal) => {
			console.log(`updating ${source} alerts`);

			const at = current_time.value;
			const query_params = at !== undefined ? `?at=${at}` : '';
			const res = await fetch(`/api/v1/alerts/${source}${query_params}`, { signal });

			if (res.headers.has('x-sw-fallback')) throw new Error('Offline');
			if (!res.ok) throw new Error('Failed to fetch alerts');

			const data: ApiAlert[] = await res.json();

			return index_alerts<S>(data);
		},
		{ alerts: [], alerts_by_route: new SvelteMap() },
		{ interval: source_info[source].refresh_interval.alerts, debounce: 500 }
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
