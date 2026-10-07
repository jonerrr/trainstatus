import { SvelteMap } from 'svelte/reactivity';

import type { Source } from '#lib/client/index.js';
import { page } from '#lib/test/page.svelte.js';

import { chartFixture, chartNow } from './fixtures.js';

export const current_time = { ms: chartNow };
export const resources: Record<string, { current: Map<string, unknown>; status: string }> = {};
export const stopResources: Record<
	string,
	{
		current: { by_trip_id: ReturnType<typeof chartFixture>['stopTimes'] };
		add_route: () => void;
		remove_route: () => void;
	}
> = {};

export function setFixture(source: Source, direction: number, patterns?: string[][]) {
	const fixture = chartFixture(source, direction, patterns);
	page.data = {
		selected_sources: [source],
		stops: { [source]: fixture.stops },
		stops_by_id: { [source]: Object.fromEntries(fixture.stops.map((s) => [s.id, s])) },
		routes: { [source]: [fixture.route] },
		routes_by_id: { [source]: { '4': fixture.route } }
	};
	resources[source] = {
		current: new SvelteMap(fixture.trips.map((t) => [t.id, t])),
		status: 'ready'
	};
	stopResources[source] = {
		current: { by_trip_id: fixture.stopTimes },
		add_route() {},
		remove_route() {}
	};
	return fixture;
}
