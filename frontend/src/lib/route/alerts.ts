import type { ApiAlert } from '#lib/client/index.js';
import type { AlertsData } from '#lib/data/alerts.js';
import type { CatalogData } from '#lib/data/decode.js';

/** A route's alerts, highest sort order first. */
export function routeAlerts(alerts: AlertsData | undefined, routeId: string): ApiAlert[] {
	const order = (alert: ApiAlert) =>
		alert.entities.find((e) => e.route_id === routeId)?.sort_order ?? 0;
	return [...(alerts?.byRoute.get(routeId) ?? [])].sort((a, b) => order(b) - order(a));
}

/** Subway express variants share the base route's alerts. */
export function alertRoutes(catalog: CatalogData) {
	return catalog.routes.filter(
		(route) => catalog.source !== 'mta_subway' || !route.id.endsWith('X')
	);
}
