import type { Catalog, RouteService, Service, Source } from '#lib/client/index.js';
import { TransitData } from '#lib/transit/store.svelte.js';

export interface TransitFixture {
	catalogs: Partial<Record<Source, Catalog>>;
	services: Partial<Record<Source, Service>>;
}

/** Fetcher that answers catalog, service and alert requests from in-memory fixtures. */
export function fixtureFetcher(fixture: TransitFixture): typeof fetch {
	const json = (body: unknown, status = 200) =>
		new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });
	return (async (input: RequestInfo | URL) => {
		const url = new URL(String(input), location.origin);
		const [, , , kind, source] = url.pathname.split('/') as [
			string,
			string,
			string,
			string,
			Source
		];
		if (kind === 'alerts') return json([]);
		if (kind === 'catalog') {
			const catalog = fixture.catalogs[source];
			return catalog ? json(catalog) : json({}, 404);
		}
		if (kind === 'service') {
			const service = fixture.services[source];
			if (!service) return json({}, 404);
			const ids = url.searchParams.get('route_ids')?.split(',');
			const empty: RouteService = {
				trips: [],
				stops: [],
				calls: { trip: [], stop: [], sequence: [], arrival: [], departure: [] }
			};
			const routes = ids
				? Object.fromEntries(ids.map((id) => [id, service.routes[id] ?? empty]))
				: service.routes;
			return json({ ...service, routes });
		}
		return json({}, 404);
	}) as typeof fetch;
}

export function fixtureTransitData(fixture: TransitFixture) {
	const sources = Object.keys(fixture.catalogs) as Source[];
	return new TransitData({ sources, fetcher: fixtureFetcher(fixture), batch_ms: 0 });
}
