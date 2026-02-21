import type { RouteStop, Stop } from '@trainstatus/client';

// TODO: double check this still works and is necessary after refactor to separate source-specific data into separate types
// Get main routes for a stop (for subway, filter to main lines only)
export const main_stop_routes = (stop: Stop): RouteStop[] => {
	if (stop.data.source === 'mta_subway') {
		// For subway stops, filter out express routes that are variants of local routes
		// Express routes typically end in 'X' or have same base ID
		const mainRoutes = new Map<string, RouteStop>();
		for (const route of stop.routes) {
			const baseId = route.route_id.replace(/X$/, '');
			if (!mainRoutes.has(baseId) || !route.route_id.endsWith('X')) {
				mainRoutes.set(baseId, route);
			}
		}
		return Array.from(mainRoutes.values());
	}
	return stop.routes;
};

export const calculate_route_height = () => 52;

export function calculate_stop_height(item: Stop) {
	let height = 44; // stop name height (28px) + 16px padding

	if (item.data.source === 'mta_bus') {
		height += item.routes.length * 56;
	} else if (item.data.source === 'mta_subway') {
		// headsign height
		height += 24;
		// route arrivals height
		height += item.routes.length * 24;
	}
	// TODO: handle other sources

	return height;
}
