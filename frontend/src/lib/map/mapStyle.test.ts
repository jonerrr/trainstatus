import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';

interface StyleLayer {
	id: string;
	type: string;
	minzoom?: number;
	'source-layer'?: string;
}

const stylePath = new URL('../../../../geo/styles/dark-matter.json', import.meta.url);
const style = JSON.parse(await readFile(stylePath, 'utf8')) as { layers: StyleLayer[] };
const layersById = new Map(style.layers.map((layer) => [layer.id, layer]));

describe('transit-first basemap style', () => {
	it('has unique layer ids and preserves transit ordering anchors', () => {
		const ids = style.layers.map((layer) => layer.id);
		expect(new Set(ids).size).toBe(ids.length);

		const anchors = ['waterway_label', 'watername_ocean', 'place_hamlet'];
		const anchorIndexes = anchors.map((id) => ids.indexOf(id));
		expect(anchorIndexes.every((index) => index >= 0)).toBe(true);
		expect(anchorIndexes).toEqual(anchorIndexes.toSorted((a, b) => a - b));
	});

	it('excludes detail that competes with transit information', () => {
		for (const id of [
			'landcover',
			'landuse_residential',
			'landuse',
			'tunnel_rail',
			'tunnel_rail_dash',
			'rail',
			'rail_dash',
			'building',
			'building-top',
			'poi_stadium',
			'poi_park',
			'housenumber'
		]) {
			expect(layersById.has(id), `${id} should be removed`).toBe(false);
		}
	});

	it('contains no building geometry or street-name labels', () => {
		const competingLayers = style.layers.filter((layer) => {
			const identity = `${layer.id} ${layer['source-layer'] ?? ''}`.toLowerCase();
			return (
				/building|housenumber/.test(identity) ||
				layer['source-layer'] === 'transportation_name' ||
				(layer.type === 'symbol' && /roadname|street/.test(identity))
			);
		});

		expect(competingLayers.map((layer) => layer.id)).toEqual([]);
	});

	it('defers local street geometry until useful zooms', () => {
		for (const id of [
			'tunnel_minor_case',
			'tunnel_minor_fill',
			'road_minor_case',
			'road_minor_fill',
			'bridge_minor_case',
			'bridge_minor_fill'
		]) {
			expect(layersById.get(id)?.minzoom, id).toBe(15);
		}

		for (const id of [
			'tunnel_service_case',
			'tunnel_service_fill',
			'tunnel_path',
			'road_service_case',
			'road_service_fill',
			'road_path',
			'bridge_service_case',
			'bridge_service_fill',
			'bridge_path'
		]) {
			expect(layersById.get(id)?.minzoom, id).toBe(17);
		}
	});
});
