import type { ApiAlert } from '#lib/client/index.js';
import icons from '#lib/icons.js';

export interface AlertsData {
	alerts: readonly ApiAlert[];
	/** Each alert appears once per affected route. */
	by_route: ReadonlyMap<string, readonly ApiAlert[]>;
}

const mta_subway_icon_regex = /(\[(.+?)\])/gm;

function parse_html(html: string) {
	return html.replaceAll(mta_subway_icon_regex, (_match, _p1, p2) => {
		const icon = icons.find((t) => t.name === p2) ?? icons[icons.length - 1];
		if (icon.complete_svg) return icon.svg;
		return `<svg xmlns="http://www.w3.org/2000/svg" class="inline-block" width="1rem" height="1rem" viewBox="0 0 90 90" focusable="false"> ${icon.svg} </svg>`;
	});
}

/** Index alerts by route and render inline subway icons; timestamps stay in Unix seconds. */
export function decodeAlerts(body: ApiAlert[]): AlertsData {
	const by_route = new Map<string, ApiAlert[]>();
	const alerts = body.map((alert): ApiAlert => {
		const decoded = {
			...alert,
			translations: alert.translations.map((t) => ({
				...t,
				text: t.format === 'html' ? parse_html(t.text) : t.text
			}))
		};
		for (const route_id of new Set(alert.entities.map((entity) => entity.route_id))) {
			let list = by_route.get(route_id);
			if (!list) by_route.set(route_id, (list = []));
			list.push(decoded);
		}
		return decoded;
	});
	return { alerts, by_route };
}
