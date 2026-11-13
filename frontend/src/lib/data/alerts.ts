import type { ApiAlert } from '#lib/client/index.js';
import icons from '#lib/icons.js';

export interface AlertsData {
	alerts: readonly ApiAlert[];
	/** Each alert appears once per affected route. */
	byRoute: ReadonlyMap<string, readonly ApiAlert[]>;
}

const mtaSubwayIconRegex = /(\[(.+?)\])/gm;

function parseHtml(html: string) {
	return html.replaceAll(mtaSubwayIconRegex, (_match, _p1, p2) => {
		const icon = icons.find((t) => t.name === p2) ?? icons[icons.length - 1];
		if (icon.completeSvg) return icon.svg;
		return `<svg xmlns="http://www.w3.org/2000/svg" class="inline-block" width="1rem" height="1rem" viewBox="0 0 90 90" focusable="false"> ${icon.svg} </svg>`;
	});
}

/** Index alerts by route and render inline subway icons; timestamps stay in Unix seconds. */
export function decodeAlerts(body: ApiAlert[]): AlertsData {
	const byRoute = new Map<string, ApiAlert[]>();
	const alerts = body.map((alert): ApiAlert => {
		const decoded = {
			...alert,
			translations: alert.translations.map((t) => ({
				...t,
				text: t.format === 'html' ? parseHtml(t.text) : t.text
			}))
		};
		for (const routeId of new Set(alert.entities.map((entity) => entity.route_id))) {
			let list = byRoute.get(routeId);
			if (!list) byRoute.set(routeId, (list = []));
			list.push(decoded);
		}
		return decoded;
	});
	return { alerts, byRoute };
}
