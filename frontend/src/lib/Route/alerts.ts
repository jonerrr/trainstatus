import type { ApiAlert } from '#lib/client/index.js';
import type { AlertsData } from '#lib/transit/alerts.js';

/** A route's alerts, highest sort order first. */
export function route_alerts(alerts: AlertsData | undefined, route_id: string): ApiAlert[] {
	const order = (alert: ApiAlert) =>
		alert.entities.find((e) => e.route_id === route_id)?.sort_order ?? 0;
	return [...(alerts?.by_route.get(route_id) ?? [])].sort((a, b) => order(b) - order(a));
}
