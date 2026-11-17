import type { Source } from '#lib/client/index.js';
import type { CatalogData } from '#lib/data/decode.js';

import type { Feature, FeatureCollection, LineString } from 'geojson';

import type { RenderUnitTable } from './trajectory.js';

export interface RouteShapeProperties {
	id: string;
	route_id: string;
	source: Source;
	short_name: string;
	long_name: string;
	color: string;
	text_color: string;
	shuttle: boolean;
}

/**
 * Route polylines for each distinct source/route/shape in the trajectory payload.
 * Feature properties retain catalog route identity for highlight and hit targets.
 */
export function buildActiveRouteShapesGeoJSON(
	tables: ReadonlyMap<Source, RenderUnitTable>,
	catalogFor: (source: Source) => CatalogData | undefined
): FeatureCollection<LineString, RouteShapeProperties> {
	const features: Feature<LineString, RouteShapeProperties>[] = [];
	for (const [source, table] of tables) {
		const catalog = catalogFor(source);
		if (!catalog) continue;
		const unitsByTrip = new Map(table.units.map((unit) => [`${unit.source}:${unit.tripId}`, unit]));
		const seenShapes = new Set<string>();
		for (const [tripKey, trip] of table.trips) {
			if (!trip.shape.positions.length) continue;
			const unit = unitsByTrip.get(tripKey);
			if (!unit) continue;
			const shapeKey = `${source}:${unit.routeId}:${trip.shape.key}`;
			if (seenShapes.has(shapeKey)) continue;
			seenShapes.add(shapeKey);
			const route = catalog.routesById.get(unit.routeId);
			const shuttle =
				route?.data.source === 'mta_bus' && route.data.service_types.includes('SHUTTLE');
			features.push({
				type: 'Feature',
				id: unit.routeId,
				properties: {
					id: unit.routeId,
					route_id: unit.routeId,
					source,
					short_name: route?.short_name ?? '',
					long_name: route?.long_name ?? '',
					color: route?.color ?? '',
					text_color: route?.text_color ?? '',
					shuttle: shuttle ?? false
				},
				geometry: {
					type: 'LineString',
					coordinates: trip.shape.positions
				}
			});
		}
	}
	return { type: 'FeatureCollection', features };
}

export type RoutesGeoJSON = FeatureCollection<LineString, RouteShapeProperties>;
