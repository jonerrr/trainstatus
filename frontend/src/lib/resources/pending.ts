interface SettledResource {
	available: boolean;
	error: Error | null;
	coversRoute?(route: string): boolean;
}
/** Skeleton while this view has no rows and the resource has not settled. */
export function awaitingRows(
	resource: SettledResource | undefined,
	rowCount: number,
	monitoredRoute?: string
): boolean {
	if (rowCount > 0) return false;
	if (!resource) return true;
	if (resource.error) return false;
	if (
		monitoredRoute !== undefined &&
		resource.coversRoute &&
		!resource.coversRoute(monitoredRoute)
	) {
		return true;
	}
	return !resource.available;
}

/** Nothing retained to show because the resource failed before publishing rows. */
export function unavailableRows(resource: SettledResource | undefined, rowCount: number): boolean {
	return rowCount === 0 && !!resource?.error;
}

type MonitoredResource = Omit<SettledResource, 'available' | 'coversRoute'> &
	Required<Pick<SettledResource, 'coversRoute'>>;

/** Skeleton for one monitored route until that route is in the published result. */
export function awaitingRoute(resource: MonitoredResource | undefined, route: string): boolean {
	return !resource?.error && !resource?.coversRoute(route);
}
